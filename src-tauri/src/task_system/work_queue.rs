/// Work queue with worker pool for processing assets
use crate::models::{Asset, ProcessingCategory};
use crate::task_system::db_writer::{DbBatchWriter, ProcessingOutput};
use crate::task_system::processor::{process_asset_cpu, process_clap_embedding_batch};
use crate::utils::unix_now;
use crate::zip_cache;
use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Notify, RwLock, Semaphore};
use tokio::time::{interval, Duration};

const BATCH_UPDATE_INTERVAL_SEC: u64 = 2;
const CLAP_BATCH_SIZE: usize = 16;
const NESTED_ZIP_BATCH_SIZE: usize = 8;

/// Tracks completion of a group of ZIP batches for staged dispatch.
/// The dispatcher waits on `done` until all batches in the group finish processing.
struct BatchGroupCompletion {
    remaining: AtomicUsize,
    done: Notify,
}

impl BatchGroupCompletion {
    fn new(count: usize) -> Self {
        Self {
            remaining: AtomicUsize::new(count),
            done: Notify::new(),
        }
    }

    fn batch_done(&self) {
        if self.remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.done.notify_one();
        }
    }
}

/// A group of batches sharing the same nested ZIP key.
struct ZipBatchGroup {
    #[cfg_attr(not(test), allow(dead_code))]
    key: String,
    batches: Vec<WorkBatch>,
}

/// Dispatch plan separating ZIP-grouped and non-ZIP batches.
struct BatchPlan {
    /// ZIP batches grouped by key, sorted by group size descending
    zip_groups: Vec<ZipBatchGroup>,
    /// Non-ZIP batches (individual assets) + CLAP batches
    non_zip: Vec<WorkBatch>,
}


// Error recording is now handled by DbBatchWriter

/// Progress statistics for a processing category
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub category: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub is_paused: bool,
    pub is_running: bool,
    // Processing details
    pub current_file: Option<String>,
    pub processing_rate: f64,
    pub eta_seconds: Option<u64>,
}

/// Internal category state tracking
#[derive(Debug, Clone)]
struct CategoryState {
    pause_signal: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    total_assets: Arc<AtomicUsize>,
    completed_assets: Arc<AtomicUsize>,
    failed_assets: Arc<AtomicUsize>,
    is_running: Arc<AtomicBool>,
    // Processing details tracking
    current_file: Arc<RwLock<Option<String>>>,
    started_at: Arc<RwLock<Option<i64>>>,
    // Concurrency limiter - limits how many workers can process this category simultaneously
    concurrency_limiter: Arc<Semaphore>,
    // Generation counter - incremented on each start to detect stale work items
    generation: Arc<AtomicU64>,
}

impl CategoryState {
    fn new(max_concurrent: usize) -> Self {
        Self {
            pause_signal: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            total_assets: Arc::new(AtomicUsize::new(0)),
            completed_assets: Arc::new(AtomicUsize::new(0)),
            failed_assets: Arc::new(AtomicUsize::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
            current_file: Arc::new(RwLock::new(None)),
            started_at: Arc::new(RwLock::new(None)),
            concurrency_limiter: Arc::new(Semaphore::new(max_concurrent)),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[derive(Clone)]
struct WorkBatch {
    category: ProcessingCategory,
    generation: u64,
    assets: Vec<Asset>,
    pre_generate_thumbnails: bool,
    /// Completion tracker for staged ZIP dispatch (None for non-ZIP / CLAP batches)
    group_completion: Option<Arc<BatchGroupCompletion>>,
}

/// Work queue manages asset processing with a worker pool
pub struct WorkQueue {
    /// High-priority channel for ZIP batches (staged dispatch, one key group at a time)
    zip_tx: Sender<WorkBatch>,
    zip_rx: Receiver<WorkBatch>,
    /// Low-priority channel for non-ZIP and CLAP batches
    nonzip_tx: Sender<WorkBatch>,
    nonzip_rx: Receiver<WorkBatch>,
    category_states: Arc<RwLock<HashMap<ProcessingCategory, CategoryState>>>,
    worker_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    dispatcher_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Batched database writer shared by all workers (created lazily on first start)
    db_writer: RwLock<Option<DbBatchWriter>>,
}

impl WorkQueue {
    pub fn new() -> Self {
        let (zip_tx, zip_rx) = unbounded();
        let (nonzip_tx, nonzip_rx) = unbounded();

        Self {
            zip_tx,
            zip_rx,
            nonzip_tx,
            nonzip_rx,
            category_states: Arc::new(RwLock::new(HashMap::new())),
            worker_handles: Arc::new(RwLock::new(Vec::new())),
            dispatcher_handle: Arc::new(RwLock::new(None)),
            db_writer: RwLock::new(None),
        }
    }

    /// Get or create the batch writer, returning a clone of the sender.
    async fn ensure_db_writer(&self, db: &SqlitePool) -> DbBatchWriter {
        let mut writer = self.db_writer.write().await;
        if writer.is_none() {
            *writer = Some(DbBatchWriter::new(db.clone()));
        }
        writer.as_ref().unwrap().clone()
    }

    /// Get or create category state
    async fn ensure_category_state(&self, category: ProcessingCategory) -> CategoryState {
        let mut states = self.category_states.write().await;
        states.entry(category).or_insert_with(|| {
            // CLAP uses 2 workers: while batch 1 is in GPU forward pass, batch 2
            // preprocesses audio on CPU (server handles concurrency via thread pool)
            // Image/Audio use many workers (CPU-bound work benefits from parallelism)
            let max_concurrent = match category {
                ProcessingCategory::Clap => 2,
                ProcessingCategory::Image | ProcessingCategory::Audio => 100, // effectively unlimited
            };
            CategoryState::new(max_concurrent)
        }).clone()
    }

    /// Build a dispatch plan that separates ZIP-grouped and non-ZIP batches.
    ///
    /// For Image/Audio: groups assets globally by nested ZIP key (HashMap, not just
    /// consecutive runs). ZIP groups are sorted by size descending so the largest
    /// groups are dispatched first.
    ///
    /// For CLAP: sorts by key then chunks with key-boundary awareness. All CLAP
    /// batches go into `non_zip` since CLAP has concurrency=1 and doesn't need
    /// staged dispatch.
    fn build_batch_plan(
        category: ProcessingCategory,
        generation: u64,
        assets: Vec<Asset>,
        pre_generate_thumbnails: bool,
    ) -> BatchPlan {
        match category {
            ProcessingCategory::Clap => {
                // Sort assets so same-key assets are consecutive
                let mut sorted = assets;
                sorted.sort_by(|a, b| {
                    let ka = zip_cache::nested_zip_group_key(a);
                    let kb = zip_cache::nested_zip_group_key(b);
                    ka.cmp(&kb)
                });

                // Build batches that respect key boundaries
                let mut batches = Vec::new();
                let mut current_batch = Vec::new();
                let mut current_key: Option<String> = None;

                for asset in sorted {
                    let key = zip_cache::nested_zip_group_key(&asset);

                    // Flush on key change or batch full
                    if !current_batch.is_empty()
                        && (key != current_key || current_batch.len() >= CLAP_BATCH_SIZE)
                    {
                        batches.push(WorkBatch {
                            category,
                            generation,
                            assets: std::mem::take(&mut current_batch),
                            pre_generate_thumbnails: false,
                            group_completion: None,
                        });
                    }

                    current_key = key;
                    current_batch.push(asset);
                }
                if !current_batch.is_empty() {
                    batches.push(WorkBatch {
                        category,
                        generation,
                        assets: current_batch,
                        pre_generate_thumbnails: false,
                        group_completion: None,
                    });
                }

                BatchPlan {
                    zip_groups: vec![],
                    non_zip: batches,
                }
            }
            ProcessingCategory::Image | ProcessingCategory::Audio => {
                // Group assets globally by nested ZIP key
                let mut zip_map: HashMap<String, Vec<Asset>> = HashMap::new();
                let mut non_zip_assets: Vec<Asset> = Vec::new();

                for asset in assets {
                    match zip_cache::nested_zip_group_key(&asset) {
                        Some(key) => zip_map.entry(key).or_default().push(asset),
                        None => non_zip_assets.push(asset),
                    }
                }

                // Create ZIP batch groups
                let mut zip_groups: Vec<ZipBatchGroup> = zip_map
                    .into_iter()
                    .map(|(key, group_assets)| {
                        let batches = group_assets
                            .chunks(NESTED_ZIP_BATCH_SIZE)
                            .map(|chunk| WorkBatch {
                                category,
                                generation,
                                assets: chunk.to_vec(),
                                pre_generate_thumbnails,
                                group_completion: None, // set by dispatcher
                            })
                            .collect();
                        ZipBatchGroup { key, batches }
                    })
                    .collect();

                // Sort groups by total asset count descending (process largest first)
                zip_groups.sort_by(|a, b| {
                    let a_total: usize = a.batches.iter().map(|b| b.assets.len()).sum();
                    let b_total: usize = b.batches.iter().map(|b| b.assets.len()).sum();
                    b_total.cmp(&a_total)
                });

                // Create non-ZIP batches (one per asset)
                let non_zip = non_zip_assets
                    .into_iter()
                    .map(|asset| WorkBatch {
                        category,
                        generation,
                        assets: vec![asset],
                        pre_generate_thumbnails,
                        group_completion: None,
                    })
                    .collect();

                BatchPlan {
                    zip_groups,
                    non_zip,
                }
            }
        }
    }

    /// Start processing assets from the work queue for a specific category
    pub async fn start(
        &self,
        category: ProcessingCategory,
        assets: Vec<Asset>,
        db: SqlitePool,
        app_handle: AppHandle,
        pre_generate_thumbnails: bool,
    ) -> Result<(), String> {
        // Get or create category state
        let state = self.ensure_category_state(category).await;

        // Check if this category is already running
        if state
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(format!("Processing for category '{}' is already running", category.as_str()));
        }

        // Reset signals and counters for this category
        state.pause_signal.store(false, Ordering::SeqCst);
        state.stop_signal.store(false, Ordering::SeqCst);
        let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
        state.total_assets.store(assets.len(), Ordering::SeqCst);
        state.completed_assets.store(0, Ordering::SeqCst);
        state.failed_assets.store(0, Ordering::SeqCst);

        // Build dispatch plan with global ZIP-locality grouping
        let plan = Self::build_batch_plan(category, generation, assets, pre_generate_thumbnails);
        let total_assets = state.total_assets.load(Ordering::SeqCst);
        println!(
            "[WorkQueue] Queueing {} ZIP groups + {} non-ZIP batches for category '{}' ({} assets)",
            plan.zip_groups.len(),
            plan.non_zip.len(),
            category.as_str(),
            total_assets
        );

        // Push non-ZIP batches to non-ZIP channel (always available to workers)
        for batch in plan.non_zip {
            self.nonzip_tx
                .send(batch)
                .map_err(|e| format!("Failed to queue non-ZIP batch: {}", e))?;
        }

        // Spawn staged dispatcher for ZIP groups (concurrent, bounded by memory budget)
        if !plan.zip_groups.is_empty() {
            let zip_tx = self.zip_tx.clone();
            let stop_signal = state.stop_signal.clone();
            let gen = state.generation.clone();

            let dispatcher = tokio::spawn(async move {
                // Allow multiple ZIP key groups in flight simultaneously,
                // bounded by how many fit in the memory budget (~1 per GB).
                let max_concurrent = std::cmp::max(
                    1,
                    zip_cache::budget_bytes() / (1024 * 1024 * 1024),
                );
                println!(
                    "[WorkQueue] Dispatcher: max {} concurrent ZIP groups (budget {:.1} GB)",
                    max_concurrent,
                    zip_cache::budget_bytes() as f64 / (1024.0 * 1024.0 * 1024.0)
                );
                let sem = Arc::new(Semaphore::new(max_concurrent));
                let mut join_set = tokio::task::JoinSet::new();

                for group in plan.zip_groups {
                    if stop_signal.load(Ordering::SeqCst)
                        || gen.load(Ordering::SeqCst) != generation
                    {
                        break;
                    }

                    let permit = match sem.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => break, // semaphore closed
                    };
                    let zip_tx = zip_tx.clone();
                    let stop = stop_signal.clone();
                    let gen_clone = gen.clone();

                    join_set.spawn(async move {
                        let _permit = permit; // held until this group finishes

                        if stop.load(Ordering::SeqCst)
                            || gen_clone.load(Ordering::SeqCst) != generation
                        {
                            return;
                        }

                        let completion =
                            Arc::new(BatchGroupCompletion::new(group.batches.len()));

                        for mut batch in group.batches {
                            batch.group_completion = Some(completion.clone());
                            if zip_tx.send(batch).is_err() {
                                return; // channel closed
                            }
                        }

                        // Wait for all batches in this group to complete
                        completion.done.notified().await;
                    });
                }

                // Wait for all dispatched groups to finish
                while join_set.join_next().await.is_some() {}
            });

            let mut dh = self.dispatcher_handle.write().await;
            *dh = Some(dispatcher);
        }

        // Check if we need to spawn workers (clean up completed workers first)
        let mut handles = self.worker_handles.write().await;
        handles.retain(|h| !h.is_finished());
        let needs_workers = handles.is_empty();

        if needs_workers {
            // Calculate number of workers based on CPU cores (leave 1 core free for system/UI)
            let num_workers = std::cmp::max(2, num_cpus::get().saturating_sub(1));
            println!("[WorkQueue] Starting {} workers (detected {} CPUs)", num_workers, num_cpus::get());

            let batch_writer = self.ensure_db_writer(&db).await;

            // Spawn workers
            for worker_id in 0..num_workers {
                let handle = self.spawn_worker(worker_id, db.clone(), batch_writer.clone()).await;
                handles.push(handle);
            }
        }
        drop(handles);

        // Spawn progress emitter task for this category
        let batch_writer_for_emitter = self.ensure_db_writer(&db).await;
        self.spawn_progress_emitter(category, app_handle.clone(), batch_writer_for_emitter).await;

        Ok(())
    }

    /// Spawn a worker task that processes assets from both channels.
    /// Workers check the ZIP channel first (priority) then fall back to non-ZIP.
    async fn spawn_worker(&self, _worker_id: usize, db: SqlitePool, batch_writer: DbBatchWriter) -> tokio::task::JoinHandle<()> {
        let zip_rx = self.zip_rx.clone();
        let nonzip_rx = self.nonzip_rx.clone();
        let category_states = self.category_states.clone();

        tokio::spawn(async move {
            loop {
                // Try ZIP channel first (high priority — staged dispatch)
                let work_batch = match zip_rx.try_recv() {
                    Ok(batch) => batch,
                    Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) => {
                        // Fall back to non-ZIP channel
                        match nonzip_rx.try_recv() {
                            Ok(batch) => batch,
                            Err(TryRecvError::Disconnected) => break,
                            Err(TryRecvError::Empty) => {
                                tokio::time::sleep(Duration::from_millis(50)).await;

                                // Check if all categories are stopped
                                let all_stopped = {
                                    let states = category_states.read().await;
                                    states.values().all(|s| !s.is_running.load(Ordering::SeqCst))
                                };

                                if all_stopped && zip_rx.is_empty() && nonzip_rx.is_empty() {
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    if zip_rx.is_empty() && nonzip_rx.is_empty() {
                                        break;
                                    }
                                }
                                continue;
                            }
                        }
                    }
                };

                let category = work_batch.category;
                let generation = work_batch.generation;
                let batch_assets = work_batch.assets;
                let pre_generate_thumbnails = work_batch.pre_generate_thumbnails;
                let group_completion = work_batch.group_completion;

                // Get category state
                let state = {
                    let states = category_states.read().await;
                    match states.get(&category) {
                        Some(s) => s.clone(),
                        None => {
                            // Signal completion even for unknown categories
                            if let Some(c) = &group_completion { c.batch_done(); }
                            continue;
                        }
                    }
                };

                // Skip stale items from a previous run
                if generation != state.generation.load(Ordering::SeqCst) {
                    if let Some(c) = &group_completion { c.batch_done(); }
                    continue;
                }

                // Check stop signal for this category
                if state.stop_signal.load(Ordering::SeqCst) {
                    if let Some(c) = &group_completion { c.batch_done(); }
                    continue;
                }

                // Wait while paused
                while state.pause_signal.load(Ordering::SeqCst) && !state.stop_signal.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                // Check again if stopped after pause
                if state.stop_signal.load(Ordering::SeqCst) {
                    if let Some(c) = &group_completion { c.batch_done(); }
                    continue;
                }

                // Acquire concurrency permit (limits parallel processing per category)
                let Ok(_permit) = state.concurrency_limiter.acquire().await else {
                    if let Some(c) = &group_completion { c.batch_done(); }
                    break;
                };

                // For CLAP, process as a batch (writes directly — SQLite serializes)
                if category == ProcessingCategory::Clap {
                    let batch_len = batch_assets.len();
                    {
                        let mut current = state.current_file.write().await;
                        *current = Some(format!("batch of {} files", batch_len));
                    }

                    let results = process_clap_embedding_batch(&batch_assets, &db).await;

                    {
                        let mut current = state.current_file.write().await;
                        *current = None;
                    }

                    for result in results {
                        if result.success {
                            state.completed_assets.fetch_add(1, Ordering::SeqCst);
                        } else {
                            state.failed_assets.fetch_add(1, Ordering::SeqCst);
                            if let Some(error_msg) = &result.error {
                                // CLAP errors written directly (SQLite serializes)
                                let _ = sqlx::query(
                                    "INSERT INTO processing_errors (asset_id, category, error_message, occurred_at, retry_count)
                                     VALUES (?, ?, ?, ?, 0)",
                                )
                                .bind(result.asset_id)
                                .bind(category.as_str())
                                .bind(error_msg)
                                .bind(unix_now())
                                .execute(&db)
                                .await;
                            }
                        }
                    }
                } else {
                    // Image/Audio: CPU-only processing, DB writes batched via DbBatchWriter
                    for asset in batch_assets {
                        while state.pause_signal.load(Ordering::SeqCst)
                            && !state.stop_signal.load(Ordering::SeqCst)
                        {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }

                        if state.stop_signal.load(Ordering::SeqCst) {
                            break;
                        }

                        {
                            let mut current = state.current_file.write().await;
                            *current = Some(asset.filename.clone());
                        }

                        let output = process_asset_cpu(&asset, pre_generate_thumbnails).await;

                        {
                            let mut current = state.current_file.write().await;
                            *current = None;
                        }

                        match &output {
                            ProcessingOutput::Failure { error, .. } => {
                                state.failed_assets.fetch_add(1, Ordering::SeqCst);
                                eprintln!(
                                    "[Worker] Failed to process asset {}: {}",
                                    asset.filename, error
                                );
                            }
                            _ => {
                                state.completed_assets.fetch_add(1, Ordering::SeqCst);
                            }
                        }

                        batch_writer.send(output).await;
                    }
                }

                // Signal group completion (for staged ZIP dispatch)
                if let Some(c) = &group_completion {
                    c.batch_done();
                }
            }
        })
    }

    /// Spawn a task that periodically emits progress events for a category
    async fn spawn_progress_emitter(&self, category: ProcessingCategory, app_handle: AppHandle, batch_writer: DbBatchWriter) {
        let state = self.ensure_category_state(category).await;
        let category_str = category.as_str().to_string();

        // Record start time
        let start_time = unix_now();
        {
            let mut started = state.started_at.write().await;
            *started = Some(start_time);
        }

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(BATCH_UPDATE_INTERVAL_SEC));

            loop {
                ticker.tick().await;

                // Check if we should stop
                if state.stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                // Get current progress
                let total = state.total_assets.load(Ordering::SeqCst);
                let completed = state.completed_assets.load(Ordering::SeqCst);
                let failed = state.failed_assets.load(Ordering::SeqCst);
                let is_paused = state.pause_signal.load(Ordering::SeqCst);
                let is_running = state.is_running.load(Ordering::SeqCst);

                // Get current file and started_at for ETA calculation
                let current_file = state.current_file.read().await.clone();
                let started_at = *state.started_at.read().await;

                // Calculate processing rate and ETA
                let (processing_rate, eta_seconds) =
                    calculate_eta(started_at, completed + failed, total, is_paused);

                // Emit progress event with category-specific event name
                let progress = ProcessingProgress {
                    category: category_str.clone(),
                    total,
                    completed,
                    failed,
                    is_paused,
                    is_running,
                    current_file,
                    processing_rate,
                    eta_seconds,
                };

                let event_name = format!("processing-progress-{}", category_str);
                let _ = app_handle.emit(&event_name, progress.clone());

                // Check if all work is done for this category
                if completed + failed >= total && total > 0 {
                    // Flush any remaining batched writes before reporting completion
                    batch_writer.flush().await;

                    println!(
                        "[Progress Emitter] Category '{}' complete: {}/{} assets processed",
                        category_str, completed, total
                    );

                    // Invalidate embedding cache when CLAP processing finishes
                    if category == ProcessingCategory::Clap {
                        crate::clap::cache::invalidate();
                    }

                    // Free unused cached nested ZIP memory (preserves pinned entries
                    // that other categories may still be using)
                    zip_cache::evict_unpinned();

                    // Mark as not running
                    state.is_running.store(false, Ordering::SeqCst);

                    // Emit final progress
                    let final_progress = ProcessingProgress {
                        category: category_str.clone(),
                        total,
                        completed,
                        failed,
                        is_paused: false,
                        is_running: false,
                        current_file: None,
                        processing_rate: 0.0,
                        eta_seconds: None,
                    };
                    let complete_event_name = format!("processing-complete-{}", category_str);
                    let _ = app_handle.emit(&complete_event_name, final_progress);

                    break;
                }
            }
        });
    }

    /// Pause processing for a specific category
    pub async fn pause(&self, category: ProcessingCategory) {
        if let Some(state) = self.category_states.read().await.get(&category) {
            state.pause_signal.store(true, Ordering::SeqCst);
        }
    }

    /// Resume processing for a specific category
    pub async fn resume(&self, category: ProcessingCategory) {
        if let Some(state) = self.category_states.read().await.get(&category) {
            state.pause_signal.store(false, Ordering::SeqCst);
        }
    }

    /// Stop processing for a specific category
    pub async fn stop(&self, category: ProcessingCategory) {
        if let Some(state) = self.category_states.read().await.get(&category) {
            state.stop_signal.store(true, Ordering::SeqCst);
            state.pause_signal.store(false, Ordering::SeqCst); // Clear pause state when stopping
            state.is_running.store(false, Ordering::SeqCst);

            // Invalidate embedding cache when CLAP processing is stopped
            if category == ProcessingCategory::Clap {
                crate::clap::cache::invalidate();
            }

            // Free unused cached nested ZIP memory (preserves pinned entries
            // that other categories may still be using)
            zip_cache::evict_unpinned();
        }

        // Abort the staged dispatcher if running
        if let Some(handle) = self.dispatcher_handle.write().await.take() {
            handle.abort();
        }

        // Check if all categories are stopped
        let all_stopped = {
            let states = self.category_states.read().await;
            states.values().all(|s| !s.is_running.load(Ordering::SeqCst))
        };

        // If all categories stopped, wait for workers to finish gracefully
        if all_stopped {
            let mut handles = self.worker_handles.write().await;
            // Workers check stop_signal and exit on their own - just wait for them
            for handle in handles.drain(..) {
                let _ = handle.await;
            }
        }
    }

    /// Get processing progress for a specific category or all categories
    pub async fn get_progress(&self, category: Option<ProcessingCategory>) -> Vec<ProcessingProgress> {
        let states = self.category_states.read().await;

        match category {
            Some(cat) => {
                // Return progress for specific category
                if let Some(state) = states.get(&cat) {
                    let total = state.total_assets.load(Ordering::SeqCst);
                    let completed = state.completed_assets.load(Ordering::SeqCst);
                    let failed = state.failed_assets.load(Ordering::SeqCst);
                    let is_paused = state.pause_signal.load(Ordering::SeqCst);

                    vec![ProcessingProgress {
                        category: cat.as_str().to_string(),
                        total,
                        completed,
                        failed,
                        is_paused,
                        is_running: state.is_running.load(Ordering::SeqCst),
                        current_file: None,
                        processing_rate: 0.0,
                        eta_seconds: None,
                    }]
                } else {
                    vec![]
                }
            }
            None => {
                // Return progress for all categories
                states
                    .iter()
                    .map(|(cat, state)| {
                        ProcessingProgress {
                            category: cat.as_str().to_string(),
                            total: state.total_assets.load(Ordering::SeqCst),
                            completed: state.completed_assets.load(Ordering::SeqCst),
                            failed: state.failed_assets.load(Ordering::SeqCst),
                            is_paused: state.pause_signal.load(Ordering::SeqCst),
                            is_running: state.is_running.load(Ordering::SeqCst),
                            current_file: None,
                            processing_rate: 0.0,
                            eta_seconds: None,
                        }
                    })
                    .collect()
            }
        }
    }

    /// Check if a specific category is currently running
    pub async fn is_running(&self, category: ProcessingCategory) -> bool {
        self.category_states
            .read()
            .await
            .get(&category)
            .map(|s| s.is_running.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Check if a specific category is paused
    pub async fn is_paused(&self, category: ProcessingCategory) -> bool {
        self.category_states
            .read()
            .await
            .get(&category)
            .map(|s| s.pause_signal.load(Ordering::SeqCst))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Test-only helpers (no AppHandle required)
// ---------------------------------------------------------------------------
#[cfg(test)]
impl WorkQueue {
    /// Start processing without an AppHandle (skips event emission).
    pub async fn start_for_test(
        &self,
        category: ProcessingCategory,
        assets: Vec<Asset>,
        db: SqlitePool,
    ) -> Result<(), String> {
        let state = self.ensure_category_state(category).await;

        if state
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(format!(
                "Category '{}' already running",
                category.as_str()
            ));
        }

        // Reset signals / counters
        state.pause_signal.store(false, Ordering::SeqCst);
        state.stop_signal.store(false, Ordering::SeqCst);
        let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
        state.total_assets.store(assets.len(), Ordering::SeqCst);
        state.completed_assets.store(0, Ordering::SeqCst);
        state.failed_assets.store(0, Ordering::SeqCst);

        let plan = Self::build_batch_plan(category, generation, assets, false);

        // Push non-ZIP batches
        for batch in plan.non_zip {
            self.nonzip_tx
                .send(batch)
                .map_err(|e| format!("Failed to queue: {}", e))?;
        }

        // Spawn staged dispatcher for ZIP groups
        if !plan.zip_groups.is_empty() {
            let zip_tx = self.zip_tx.clone();
            let stop_signal = state.stop_signal.clone();
            let gen = state.generation.clone();

            let dispatcher = tokio::spawn(async move {
                for group in plan.zip_groups {
                    if stop_signal.load(Ordering::SeqCst)
                        || gen.load(Ordering::SeqCst) != generation
                    {
                        break;
                    }

                    let completion = Arc::new(BatchGroupCompletion::new(group.batches.len()));

                    for mut batch in group.batches {
                        batch.group_completion = Some(completion.clone());
                        if zip_tx.send(batch).is_err() {
                            return;
                        }
                    }

                    completion.done.notified().await;
                }
            });

            let mut dh = self.dispatcher_handle.write().await;
            *dh = Some(dispatcher);
        }

        // Spawn workers (clean up stale handles first)
        let mut handles = self.worker_handles.write().await;
        handles.retain(|h| !h.is_finished());
        if handles.is_empty() {
            let batch_writer = self.ensure_db_writer(&db).await;
            let num_workers = 2; // fewer workers in tests
            for worker_id in 0..num_workers {
                let handle = self.spawn_worker(worker_id, db.clone(), batch_writer.clone()).await;
                handles.push(handle);
            }
        }
        drop(handles);

        // Completion monitor instead of progress emitter
        self.spawn_completion_monitor(category).await;
        Ok(())
    }

    /// Sets `is_running = false` when all items in a category are processed.
    /// Also flushes the batch writer to ensure all DB writes complete.
    async fn spawn_completion_monitor(&self, category: ProcessingCategory) {
        let state = self.ensure_category_state(category).await;
        let batch_writer = {
            let w = self.db_writer.read().await;
            w.clone()
        };
        tokio::spawn(async move {
            loop {
                if state.stop_signal.load(Ordering::SeqCst) {
                    break;
                }
                let total = state.total_assets.load(Ordering::SeqCst);
                let completed = state.completed_assets.load(Ordering::SeqCst);
                let failed = state.failed_assets.load(Ordering::SeqCst);
                if total > 0 && completed + failed >= total {
                    // Flush batched writes before marking complete
                    if let Some(ref writer) = batch_writer {
                        writer.flush().await;
                    }
                    state.is_running.store(false, Ordering::SeqCst);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
    }

    /// Block until a category finishes or `timeout` elapses. Returns `true` on
    /// completion, `false` on timeout (possible deadlock).
    pub async fn wait_for_category_completion(
        &self,
        category: ProcessingCategory,
        timeout: Duration,
    ) -> bool {
        let start = std::time::Instant::now();
        loop {
            let progress = self.get_progress(Some(category)).await;
            if let Some(p) = progress.first() {
                if p.total > 0 && p.completed + p.failed >= p.total {
                    return true;
                }
            }
            if start.elapsed() > timeout {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Calculate processing rate and ETA
fn calculate_eta(
    started_at: Option<i64>,
    processed: usize,
    total: usize,
    is_paused: bool,
) -> (f64, Option<u64>) {
    if is_paused || processed == 0 {
        return (0.0, None);
    }

    let Some(start) = started_at else {
        return (0.0, None);
    };

    let now = unix_now();

    let elapsed_secs = (now - start) as f64;
    if elapsed_secs <= 0.0 {
        return (0.0, None);
    }

    let rate = processed as f64 / elapsed_secs;
    let remaining = total.saturating_sub(processed);

    if rate > 0.0 {
        let eta = (remaining as f64 / rate).ceil() as u64;
        (rate, Some(eta))
    } else {
        (0.0, None)
    }
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    async fn setup_image_assets(
        dir: &std::path::Path,
        db: &SqlitePool,
        count: usize,
    ) -> Vec<Asset> {
        let folder_path = dir.to_string_lossy().replace('\\', "/");
        let folder_id = insert_source_folder(db, &folder_path, "test").await;
        let mut assets = Vec::new();
        for i in 0..count {
            let name = format!("img_{}.png", i);
            create_test_png(dir, &name);
            let mut asset = make_asset(&name, folder_id, "", "image", "png");
            asset.folder_path = folder_path.clone();
            asset.id = insert_asset(db, &asset).await;
            assets.push(asset);
        }
        assets
    }

    async fn setup_audio_assets(
        dir: &std::path::Path,
        db: &SqlitePool,
        count: usize,
    ) -> Vec<Asset> {
        let folder_path = dir.to_string_lossy().replace('\\', "/");
        // Reuse existing folder or create new one (INSERT OR IGNORE + SELECT)
        sqlx::query("INSERT OR IGNORE INTO source_folders (path, label, added_at) VALUES (?, 'test_audio', 1000000)")
            .bind(&folder_path)
            .execute(db)
            .await
            .unwrap();
        let folder_id: i64 = sqlx::query_scalar("SELECT id FROM source_folders WHERE path = ?")
            .bind(&folder_path)
            .fetch_one(db)
            .await
            .unwrap();
        let mut assets = Vec::new();
        for i in 0..count {
            let name = format!("audio_{}.wav", i);
            create_test_wav(dir, &name);
            let mut asset = make_asset(&name, folder_id, "", "audio", "wav");
            asset.folder_path = folder_path.clone();
            asset.id = insert_asset(db, &asset).await;
            assets.push(asset);
        }
        assets
    }

    fn make_nested_batch_asset(zip_file: &str, zip_entry: &str, name: &str) -> Asset {
        let mut asset = make_asset(name, 1, "", "audio", "wav");
        asset.folder_path = "D:/Assets".to_string();
        asset.zip_file = Some(zip_file.to_string());
        asset.zip_entry = Some(zip_entry.to_string());
        asset
    }

    // -----------------------------------------------------------------------
    // calculate_eta (pure function)
    // -----------------------------------------------------------------------

    #[test]
    fn test_calculate_eta_paused_returns_zero() {
        let (rate, eta) = calculate_eta(Some(100), 50, 100, true);
        assert_eq!(rate, 0.0);
        assert!(eta.is_none());
    }

    #[test]
    fn test_calculate_eta_no_progress_returns_zero() {
        let (rate, eta) = calculate_eta(Some(100), 0, 100, false);
        assert_eq!(rate, 0.0);
        assert!(eta.is_none());
    }

    #[test]
    fn test_calculate_eta_no_start_time_returns_zero() {
        let (rate, eta) = calculate_eta(None, 50, 100, false);
        assert_eq!(rate, 0.0);
        assert!(eta.is_none());
    }

    #[test]
    fn test_batch_plan_separates_zip_and_nonzip() {
        let assets = vec![
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/one.wav", "one.wav"),
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/two.wav", "two.wav"),
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/three.wav", "three.wav"),
            make_asset("plain.wav", 1, "", "audio", "wav"),
            make_nested_batch_asset("pack_b.zip", "inner_b.zip/four.wav", "four.wav"),
            make_nested_batch_asset("pack_b.zip", "inner_b.zip/five.wav", "five.wav"),
        ];

        let plan = WorkQueue::build_batch_plan(ProcessingCategory::Audio, 7, assets, false);

        // 2 ZIP groups (pack_a with 3 assets, pack_b with 2)
        assert_eq!(plan.zip_groups.len(), 2);
        // Sorted by size descending: pack_a (3) before pack_b (2)
        assert_eq!(plan.zip_groups[0].batches.iter().map(|b| b.assets.len()).sum::<usize>(), 3);
        assert_eq!(plan.zip_groups[1].batches.iter().map(|b| b.assets.len()).sum::<usize>(), 2);

        // 1 non-ZIP batch (plain.wav)
        assert_eq!(plan.non_zip.len(), 1);
        assert_eq!(plan.non_zip[0].assets.len(), 1);

        // All batches have correct generation/category
        for group in &plan.zip_groups {
            for batch in &group.batches {
                assert_eq!(batch.generation, 7);
                assert_eq!(batch.category, ProcessingCategory::Audio);
            }
        }
    }

    #[test]
    fn test_batch_plan_groups_non_consecutive_same_key() {
        // Assets from the same ZIP key are interleaved with other keys
        let assets = vec![
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/one.wav", "one.wav"),
            make_nested_batch_asset("pack_b.zip", "inner_b.zip/two.wav", "two.wav"),
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/three.wav", "three.wav"),
            make_nested_batch_asset("pack_b.zip", "inner_b.zip/four.wav", "four.wav"),
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/five.wav", "five.wav"),
        ];

        let plan = WorkQueue::build_batch_plan(ProcessingCategory::Audio, 1, assets, false);

        // Both ZIP groups should exist with ALL their assets (globally grouped)
        assert_eq!(plan.zip_groups.len(), 2);
        let total_zip_assets: usize = plan.zip_groups.iter()
            .flat_map(|g| &g.batches)
            .map(|b| b.assets.len())
            .sum();
        assert_eq!(total_zip_assets, 5);
        // pack_a has 3, pack_b has 2
        assert_eq!(plan.zip_groups[0].batches.iter().map(|b| b.assets.len()).sum::<usize>(), 3);
        assert_eq!(plan.zip_groups[1].batches.iter().map(|b| b.assets.len()).sum::<usize>(), 2);
        assert_eq!(plan.non_zip.len(), 0);
    }

    #[test]
    fn test_batch_plan_splits_large_zip_groups() {
        let assets: Vec<Asset> = (0..10)
            .map(|i| {
                make_nested_batch_asset(
                    "pack_a.zip",
                    &format!("inner_a.zip/{}.wav", i),
                    &format!("{}.wav", i),
                )
            })
            .collect();

        let plan = WorkQueue::build_batch_plan(ProcessingCategory::Audio, 3, assets, false);

        assert_eq!(plan.zip_groups.len(), 1);
        let batch_sizes: Vec<usize> = plan.zip_groups[0].batches.iter().map(|b| b.assets.len()).collect();
        assert_eq!(batch_sizes, vec![NESTED_ZIP_BATCH_SIZE, 2]);
    }

    #[test]
    fn test_batch_plan_clap_respects_key_boundaries() {
        let assets = vec![
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/one.wav", "one.wav"),
            make_nested_batch_asset("pack_a.zip", "inner_a.zip/two.wav", "two.wav"),
            make_nested_batch_asset("pack_b.zip", "inner_b.zip/three.wav", "three.wav"),
            make_asset("plain.wav", 1, "", "audio", "wav"),
        ];

        let plan = WorkQueue::build_batch_plan(ProcessingCategory::Clap, 1, assets, false);

        // CLAP: all batches go to non_zip (no staged dispatch needed)
        assert_eq!(plan.zip_groups.len(), 0);

        // No batch should contain assets from different ZIP keys
        for batch in &plan.non_zip {
            let keys: Vec<_> = batch.assets.iter()
                .map(|a| zip_cache::nested_zip_group_key(a))
                .collect();
            let first = &keys[0];
            assert!(keys.iter().all(|k| k == first),
                "CLAP batch should not cross key boundaries: {:?}", keys);
        }
    }

    #[test]
    fn test_batch_group_completion_tracking() {
        let completion = Arc::new(BatchGroupCompletion::new(3));

        // First two decrements should not notify
        completion.batch_done();
        assert_eq!(completion.remaining.load(Ordering::SeqCst), 2);
        completion.batch_done();
        assert_eq!(completion.remaining.load(Ordering::SeqCst), 1);
        // Third decrement hits zero (notify_one called internally)
        completion.batch_done();
        assert_eq!(completion.remaining.load(Ordering::SeqCst), 0);
    }

    // -----------------------------------------------------------------------
    // WorkQueue – basic processing
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_processes_all_images() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_image_assets(dir.path(), &db, 5).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await
            .unwrap();

        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Image, Duration::from_secs(30))
            .await;
        assert!(ok, "Image processing should complete within timeout");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM image_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_queue_processes_all_audio() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_audio_assets(dir.path(), &db, 3).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Audio, assets, db.clone())
            .await
            .unwrap();

        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Audio, Duration::from_secs(30))
            .await;
        assert!(ok, "Audio processing should complete within timeout");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audio_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }

    // -----------------------------------------------------------------------
    // Progress tracking
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_progress_tracking() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_image_assets(dir.path(), &db, 3).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await
            .unwrap();

        // While running, total should be set
        let progress = queue.get_progress(Some(ProcessingCategory::Image)).await;
        assert_eq!(progress.len(), 1);
        assert_eq!(progress[0].total, 3);

        queue
            .wait_for_category_completion(ProcessingCategory::Image, Duration::from_secs(30))
            .await;

        let progress = queue.get_progress(Some(ProcessingCategory::Image)).await;
        assert_eq!(progress[0].completed + progress[0].failed, 3);
    }

    // -----------------------------------------------------------------------
    // Pause / resume
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_pause_resume() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_image_assets(dir.path(), &db, 10).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await
            .unwrap();

        queue.pause(ProcessingCategory::Image).await;
        assert!(queue.is_paused(ProcessingCategory::Image).await);

        queue.resume(ProcessingCategory::Image).await;
        assert!(!queue.is_paused(ProcessingCategory::Image).await);

        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Image, Duration::from_secs(30))
            .await;
        assert!(ok, "Should complete after resume");
    }

    // -----------------------------------------------------------------------
    // Stop
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_stop() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_image_assets(dir.path(), &db, 50).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        queue.stop(ProcessingCategory::Image).await;

        assert!(!queue.is_running(ProcessingCategory::Image).await);
    }

    // -----------------------------------------------------------------------
    // Sequential categories (regression: stale worker handles)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_sequential_categories_respawns_workers() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;

        let image_assets = setup_image_assets(dir.path(), &db, 3).await;
        let audio_assets = setup_audio_assets(dir.path(), &db, 3).await;

        let queue = WorkQueue::new();

        // --- first category ---
        queue
            .start_for_test(ProcessingCategory::Image, image_assets, db.clone())
            .await
            .unwrap();
        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Image, Duration::from_secs(30))
            .await;
        assert!(ok, "Image processing should complete");

        // Give workers time to exit
        tokio::time::sleep(Duration::from_millis(300)).await;

        // --- second category (workers must respawn) ---
        queue
            .start_for_test(ProcessingCategory::Audio, audio_assets, db.clone())
            .await
            .unwrap();
        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Audio, Duration::from_secs(30))
            .await;
        assert!(ok, "Audio processing should complete after images (workers must respawn)");

        let (img_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM image_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        let (audio_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audio_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(img_count, 3);
        assert_eq!(audio_count, 3);
    }

    // -----------------------------------------------------------------------
    // Double-start rejected
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_double_start_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_image_assets(dir.path(), &db, 5).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets.clone(), db.clone())
            .await
            .unwrap();

        let err = queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await;
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("already running"));

        queue.stop(ProcessingCategory::Image).await;
    }

    // -----------------------------------------------------------------------
    // Failed assets are tracked
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_failed_assets_tracked() {
        let db = create_test_db().await;
        let folder_id = insert_source_folder(&db, "/nonexistent/path", "missing").await;

        // Assets pointing to non-existent files
        let mut assets = Vec::new();
        for i in 0..3 {
            let name = format!("missing_{}.png", i);
            let mut asset = make_asset(&name, folder_id, "", "image", "png");
            asset.folder_path = "/nonexistent/path".to_string();
            asset.id = insert_asset(&db, &asset).await;
            assets.push(asset);
        }

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await
            .unwrap();

        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Image, Duration::from_secs(30))
            .await;
        assert!(ok, "Even failed processing should finish");

        let progress = queue.get_progress(Some(ProcessingCategory::Image)).await;
        assert_eq!(progress[0].failed, 3);
        assert_eq!(progress[0].completed, 0);

        let (err_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM processing_errors")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(err_count, 3);
    }

    // -----------------------------------------------------------------------
    // Deadlock detection (large batch with timeout)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_queue_no_deadlock_under_load() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let assets = setup_image_assets(dir.path(), &db, 20).await;

        let queue = WorkQueue::new();
        queue
            .start_for_test(ProcessingCategory::Image, assets, db.clone())
            .await
            .unwrap();

        let ok = queue
            .wait_for_category_completion(ProcessingCategory::Image, Duration::from_secs(60))
            .await;
        assert!(ok, "Processing 20 images must not deadlock");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM image_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 20);
    }
}
