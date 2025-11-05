/// Work queue with worker pool for processing assets
use crate::models::{Asset, ProcessingCategory};
use crate::task_system::processor::{process_asset, ProcessingResult};
use crossbeam::channel::{unbounded, Receiver, Sender};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

const BATCH_UPDATE_INTERVAL_SEC: u64 = 2;
const EMIT_PROGRESS_EVERY_N_ASSETS: usize = 10;

/// Progress statistics for a processing category
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub category: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub is_paused: bool,
    pub is_running: bool,
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
}

impl CategoryState {
    fn new() -> Self {
        Self {
            pause_signal: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            total_assets: Arc::new(AtomicUsize::new(0)),
            completed_assets: Arc::new(AtomicUsize::new(0)),
            failed_assets: Arc::new(AtomicUsize::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Work queue manages asset processing with a worker pool
pub struct WorkQueue {
    work_tx: Sender<Asset>,
    work_rx: Receiver<Asset>,
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
        states.entry(category).or_insert_with(CategoryState::new).clone()
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
        state.total_assets.store(assets.len(), Ordering::SeqCst);
        state.completed_assets.store(0, Ordering::SeqCst);
        state.failed_assets.store(0, Ordering::SeqCst);

        // Queue all assets
        for asset in assets {
            self.work_tx
                .send(asset)
                .map_err(|e| format!("Failed to queue asset: {}", e))?;
        }

        // Check if we need to spawn workers (first category being started)
        let handles = self.worker_handles.read().await;
        let needs_workers = handles.is_empty();
        drop(handles);

        if needs_workers {
            // Calculate number of workers based on CPU cores (leave 1 core free for system/UI)
            let num_workers = std::cmp::max(2, num_cpus::get().saturating_sub(1));
            println!("[WorkQueue] Starting {} workers (detected {} CPUs)", num_workers, num_cpus::get());

            // Spawn workers
            let mut handles = Vec::new();
            for worker_id in 0..num_workers {
                let handle = self.spawn_worker(worker_id, db.clone(), app_handle.clone()).await;
                handles.push(handle);
            }

            // Store worker handles
            *self.worker_handles.write().await = handles;
        }

        // Spawn progress emitter task for this category
        self.spawn_progress_emitter(category, app_handle.clone()).await;

        Ok(())
    }

    /// Spawn a worker task that processes assets from the queue
    async fn spawn_worker(&self, worker_id: usize, db: SqlitePool, _app_handle: AppHandle) -> tokio::task::JoinHandle<()> {
        let work_rx = self.work_rx.clone();
        let category_states = self.category_states.clone();

        tokio::spawn(async move {
            println!("[Worker {}] Started", worker_id);

            // Buffer for batch results (for potential batch DB updates in future)
            let mut results_buffer: Vec<ProcessingResult> = Vec::with_capacity(20);

            loop {
                // Try to get work from queue (non-blocking)
                match work_rx.try_recv() {
                    Ok(asset) => {
                        // Determine asset category
                        let category = match asset.asset_type.as_str() {
                            "image" => ProcessingCategory::Image,
                            "audio" => ProcessingCategory::Audio,
                            _ => {
                                println!("[Worker {}] Unknown asset type: {}", worker_id, asset.asset_type);
                                continue;
                            }
                        };

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

                        // Process the asset
                        let result = process_asset(&asset, &db).await;

                        // Update category-specific counters
                        if result.success {
                            state.completed_assets.fetch_add(1, Ordering::SeqCst);
                        } else {
                            state.failed_assets.fetch_add(1, Ordering::SeqCst);
                            println!(
                                "[Worker {}] Failed to process asset {}: {:?}",
                                worker_id, asset.filename, result.error
                            );
                        }

                        results_buffer.push(result);

                        // Clear buffer periodically
                        if results_buffer.len() >= EMIT_PROGRESS_EVERY_N_ASSETS {
                            results_buffer.clear();
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

                // Emit progress event with category-specific event name
                let progress = ProcessingProgress {
                    category: category_str.clone(),
                    total,
                    completed,
                    failed,
                    is_paused,
                    is_running,
                };

                let event_name = format!("processing-progress-{}", category_str);
                let _ = app_handle.emit(&event_name, progress.clone());

                // Check if all work is done for this category
                if completed + failed >= total && total > 0 {
                    println!(
                        "[Progress Emitter] Category '{}' complete: {}/{} assets processed",
                        category_str, completed, total
                    );

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
        }

        // Check if all categories are stopped
        let all_stopped = {
            let states = self.category_states.read().await;
            states.values().all(|s| !s.is_running.load(Ordering::SeqCst))
        };

        // If all categories stopped, abort all workers and clear handles
        if all_stopped {
            let mut handles = self.worker_handles.write().await;
            for handle in handles.iter() {
                handle.abort();
            }
            // Clear handles so new workers can be spawned on next start
            handles.clear();
        }
    }

    /// Get processing progress for a specific category or all categories
    pub async fn get_progress(&self, category: Option<ProcessingCategory>) -> Vec<ProcessingProgress> {
        let states = self.category_states.read().await;

        match category {
            Some(cat) => {
                // Return progress for specific category
                if let Some(state) = states.get(&cat) {
                    vec![ProcessingProgress {
                        category: cat.as_str().to_string(),
                        total: state.total_assets.load(Ordering::SeqCst),
                        completed: state.completed_assets.load(Ordering::SeqCst),
                        failed: state.failed_assets.load(Ordering::SeqCst),
                        is_paused: state.pause_signal.load(Ordering::SeqCst),
                        is_running: state.is_running.load(Ordering::SeqCst),
                    }]
                } else {
                    vec![]
                }
            }
            None => {
                // Return progress for all categories
                states
                    .iter()
                    .map(|(cat, state)| ProcessingProgress {
                        category: cat.as_str().to_string(),
                        total: state.total_assets.load(Ordering::SeqCst),
                        completed: state.completed_assets.load(Ordering::SeqCst),
                        failed: state.failed_assets.load(Ordering::SeqCst),
                        is_paused: state.pause_signal.load(Ordering::SeqCst),
                        is_running: state.is_running.load(Ordering::SeqCst),
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
