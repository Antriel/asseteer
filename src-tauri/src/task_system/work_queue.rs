/// Work queue with worker pool for processing assets
use crate::models::{Asset, ProcessingCategory};
use crate::task_system::processor::{process_asset, process_clap_embedding_batch};
use crate::zip_cache;
use crossbeam::channel::{unbounded, Receiver, Sender};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{interval, Duration};

const BATCH_UPDATE_INTERVAL_SEC: u64 = 2;

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

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

/// Work item containing category, generation, and asset
type WorkItem = (ProcessingCategory, u64, Asset);

/// Work queue manages asset processing with a worker pool
pub struct WorkQueue {
    work_tx: Sender<WorkItem>,
    work_rx: Receiver<WorkItem>,
    category_states: Arc<RwLock<HashMap<ProcessingCategory, CategoryState>>>,
    worker_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl WorkQueue {
    pub fn new() -> Self {
        let (work_tx, work_rx) = unbounded();

        Self {
            work_tx,
            work_rx,
            category_states: Arc::new(RwLock::new(HashMap::new())),
            worker_handles: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get or create category state
    async fn ensure_category_state(&self, category: ProcessingCategory) -> CategoryState {
        let mut states = self.category_states.write().await;
        states.entry(category).or_insert_with(|| {
            // CLAP uses single worker (model inference is internally parallelized)
            // Image/Audio use many workers (CPU-bound work benefits from parallelism)
            let max_concurrent = match category {
                ProcessingCategory::Clap => 1,
                ProcessingCategory::Image | ProcessingCategory::Audio => 100, // effectively unlimited
            };
            CategoryState::new(max_concurrent)
        }).clone()
    }

    /// Start processing assets from the work queue for a specific category
    pub async fn start(
        &self,
        category: ProcessingCategory,
        assets: Vec<Asset>,
        db: SqlitePool,
        app_handle: AppHandle,
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

        // Queue all assets with their category and generation
        for asset in assets {
            self.work_tx
                .send((category, generation, asset))
                .map_err(|e| format!("Failed to queue asset: {}", e))?;
        }

        // Check if we need to spawn workers (clean up completed workers first)
        let mut handles = self.worker_handles.write().await;
        handles.retain(|h| !h.is_finished());
        let needs_workers = handles.is_empty();

        if needs_workers {
            // Calculate number of workers based on CPU cores (leave 1 core free for system/UI)
            let num_workers = std::cmp::max(2, num_cpus::get().saturating_sub(1));
            println!("[WorkQueue] Starting {} workers (detected {} CPUs)", num_workers, num_cpus::get());

            // Spawn workers
            for worker_id in 0..num_workers {
                let handle = self.spawn_worker(worker_id, db.clone()).await;
                handles.push(handle);
            }
        }
        drop(handles);

        // Spawn progress emitter task for this category
        self.spawn_progress_emitter(category, app_handle.clone()).await;

        Ok(())
    }

    /// Spawn a worker task that processes assets from the queue
    async fn spawn_worker(&self, worker_id: usize, db: SqlitePool) -> tokio::task::JoinHandle<()> {
        let work_rx = self.work_rx.clone();
        let category_states = self.category_states.clone();

        tokio::spawn(async move {
            println!("[Worker {}] Started", worker_id);

            loop {
                // Try to get work from queue (non-blocking)
                match work_rx.try_recv() {
                    Ok((category, generation, asset)) => {
                        // Get category state
                        let state = {
                            let states = category_states.read().await;
                            match states.get(&category) {
                                Some(s) => s.clone(),
                                None => {
                                    // Category not initialized, skip this asset
                                    println!("[Worker {}] Category {:?} not initialized", worker_id, category);
                                    continue;
                                }
                            }
                        };

                        // Skip stale items from a previous run
                        if generation != state.generation.load(Ordering::SeqCst) {
                            continue;
                        }

                        // Check stop signal for this category
                        if state.stop_signal.load(Ordering::SeqCst) {
                            // Category stopped, skip this asset
                            println!("[Worker {}] Skipping asset (category {:?} stopped)", worker_id, category);
                            continue;
                        }

                        // Wait while paused
                        while state.pause_signal.load(Ordering::SeqCst) && !state.stop_signal.load(Ordering::SeqCst) {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }

                        // Check again if stopped after pause
                        if state.stop_signal.load(Ordering::SeqCst) {
                            println!("[Worker {}] Skipping asset (category {:?} stopped after pause)", worker_id, category);
                            continue;
                        }

                        // Acquire concurrency permit (limits parallel processing per category)
                        let Ok(_permit) = state.concurrency_limiter.acquire().await else {
                            println!("[Worker {}] Semaphore closed, stopping", worker_id);
                            break;
                        };

                        // For CLAP, collect a batch before processing
                        if category == ProcessingCategory::Clap {
                            let mut batch = vec![asset];
                            const CLAP_BATCH_SIZE: usize = 8;

                            // Try to collect more items for the batch
                            while batch.len() < CLAP_BATCH_SIZE {
                                match work_rx.try_recv() {
                                    Ok((cat, gen, next_asset)) => {
                                        if cat == ProcessingCategory::Clap
                                            && gen == state.generation.load(Ordering::SeqCst)
                                            && !state.stop_signal.load(Ordering::SeqCst)
                                        {
                                            batch.push(next_asset);
                                        }
                                        // If different category/generation, we can't put it back
                                        // in a crossbeam channel easily, so just skip
                                        // (this is fine since CLAP runs with concurrency=1)
                                    }
                                    Err(_) => break,
                                }
                            }

                            let batch_len = batch.len();
                            {
                                let mut current = state.current_file.write().await;
                                *current = Some(format!("batch of {} files", batch_len));
                            }

                            let results = process_clap_embedding_batch(&batch, &db).await;

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
                                        let now = unix_now();
                                        let _ = sqlx::query(
                                            "INSERT INTO processing_errors (asset_id, category, error_message, occurred_at, retry_count)
                                             VALUES (?, ?, ?, ?, 0)"
                                        )
                                        .bind(result.asset_id)
                                        .bind(category.as_str())
                                        .bind(error_msg)
                                        .bind(now)
                                        .execute(&db)
                                        .await;
                                    }
                                }
                            }
                        } else {
                            // Non-CLAP: process one at a time as before
                            {
                                let mut current = state.current_file.write().await;
                                *current = Some(asset.filename.clone());
                            }

                            let result = process_asset(&asset, &db).await;

                            {
                                let mut current = state.current_file.write().await;
                                *current = None;
                            }

                            if result.success {
                                state.completed_assets.fetch_add(1, Ordering::SeqCst);
                            } else {
                                state.failed_assets.fetch_add(1, Ordering::SeqCst);
                                println!(
                                    "[Worker {}] Failed to process asset {}: {:?}",
                                    worker_id, asset.filename, result.error
                                );

                                if let Some(error_msg) = &result.error {
                                    let now = unix_now();
                                    let _ = sqlx::query(
                                        "INSERT INTO processing_errors (asset_id, category, error_message, occurred_at, retry_count)
                                         VALUES (?, ?, ?, ?, 0)"
                                    )
                                    .bind(asset.id)
                                    .bind(category.as_str())
                                    .bind(error_msg)
                                    .bind(now)
                                    .execute(&db)
                                    .await;
                                }
                            }
                        }
                    }
                    Err(crossbeam::channel::TryRecvError::Empty) => {
                        // No work available, wait a bit
                        tokio::time::sleep(Duration::from_millis(50)).await;

                        // Check if all categories are stopped
                        let all_stopped = {
                            let states = category_states.read().await;
                            states.values().all(|s| !s.is_running.load(Ordering::SeqCst))
                        };

                        if all_stopped && work_rx.is_empty() {
                            // Give a small grace period for more work to arrive
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            if work_rx.is_empty() {
                                println!("[Worker {}] No more work and all categories stopped, exiting", worker_id);
                                break;
                            }
                        }
                    }
                    Err(crossbeam::channel::TryRecvError::Disconnected) => {
                        println!("[Worker {}] Channel disconnected, exiting", worker_id);
                        break;
                    }
                }
            }

            println!("[Worker {}] Finished", worker_id);
        })
    }

    /// Spawn a task that periodically emits progress events for a category
    async fn spawn_progress_emitter(&self, category: ProcessingCategory, app_handle: AppHandle) {
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
                    println!(
                        "[Progress Emitter] Category '{}' complete: {}/{} assets processed",
                        category_str, completed, total
                    );

                    // Invalidate embedding cache when CLAP processing finishes
                    if category == ProcessingCategory::Clap {
                        crate::clap::cache::invalidate();
                    }

                    // Free cached nested ZIP memory
                    zip_cache::clear();

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

            // Free cached nested ZIP memory
            zip_cache::clear();
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

        for asset in assets {
            self.work_tx
                .send((category, generation, asset))
                .map_err(|e| format!("Failed to queue: {}", e))?;
        }

        // Spawn workers (clean up stale handles first)
        let mut handles = self.worker_handles.write().await;
        handles.retain(|h| !h.is_finished());
        if handles.is_empty() {
            let num_workers = 2; // fewer workers in tests
            for worker_id in 0..num_workers {
                let handle = self.spawn_worker(worker_id, db.clone()).await;
                handles.push(handle);
            }
        }
        drop(handles);

        // Completion monitor instead of progress emitter
        self.spawn_completion_monitor(category).await;
        Ok(())
    }

    /// Sets `is_running = false` when all items in a category are processed.
    async fn spawn_completion_monitor(&self, category: ProcessingCategory) {
        let state = self.ensure_category_state(category).await;
        tokio::spawn(async move {
            loop {
                if state.stop_signal.load(Ordering::SeqCst) {
                    break;
                }
                let total = state.total_assets.load(Ordering::SeqCst);
                let completed = state.completed_assets.load(Ordering::SeqCst);
                let failed = state.failed_assets.load(Ordering::SeqCst);
                if total > 0 && completed + failed >= total {
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
