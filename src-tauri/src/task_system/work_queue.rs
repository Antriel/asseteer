/// Work queue with worker pool for processing assets
use crate::models::Asset;
use crate::task_system::processor::{process_asset, ProcessingResult};
use crossbeam::channel::{unbounded, Receiver, Sender};
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

const NUM_WORKERS: usize = 4;
const BATCH_UPDATE_INTERVAL_SEC: u64 = 2;
const EMIT_PROGRESS_EVERY_N_ASSETS: usize = 10;

/// Progress statistics for the processing job
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub is_paused: bool,
    pub is_running: bool,
}

/// Work queue manages asset processing with a worker pool
pub struct WorkQueue {
    work_tx: Sender<Asset>,
    work_rx: Receiver<Asset>,
    pause_signal: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    total_assets: Arc<AtomicUsize>,
    completed_assets: Arc<AtomicUsize>,
    failed_assets: Arc<AtomicUsize>,
    is_running: Arc<AtomicBool>,
    worker_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl WorkQueue {
    pub fn new() -> Self {
        let (work_tx, work_rx) = unbounded();

        Self {
            work_tx,
            work_rx,
            pause_signal: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            total_assets: Arc::new(AtomicUsize::new(0)),
            completed_assets: Arc::new(AtomicUsize::new(0)),
            failed_assets: Arc::new(AtomicUsize::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
            worker_handles: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start processing assets from the work queue
    pub async fn start(
        &self,
        assets: Vec<Asset>,
        db: SqlitePool,
        app_handle: AppHandle,
    ) -> Result<(), String> {
        // Check if already running
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Processing is already running".to_string());
        }

        // Reset signals and counters
        self.pause_signal.store(false, Ordering::SeqCst);
        self.stop_signal.store(false, Ordering::SeqCst);
        self.total_assets.store(assets.len(), Ordering::SeqCst);
        self.completed_assets.store(0, Ordering::SeqCst);
        self.failed_assets.store(0, Ordering::SeqCst);

        // Queue all assets
        for asset in assets {
            self.work_tx
                .send(asset)
                .map_err(|e| format!("Failed to queue asset: {}", e))?;
        }

        // Spawn workers
        let mut handles = Vec::new();
        for worker_id in 0..NUM_WORKERS {
            let handle = self.spawn_worker(worker_id, db.clone(), app_handle.clone());
            handles.push(handle);
        }

        // Store worker handles
        *self.worker_handles.write().await = handles;

        // Spawn progress emitter task
        self.spawn_progress_emitter(app_handle.clone());

        Ok(())
    }

    /// Spawn a worker task that processes assets from the queue
    fn spawn_worker(&self, worker_id: usize, db: SqlitePool, _app_handle: AppHandle) -> tokio::task::JoinHandle<()> {
        let work_rx = self.work_rx.clone();
        let pause_signal = self.pause_signal.clone();
        let stop_signal = self.stop_signal.clone();
        let completed_assets = self.completed_assets.clone();
        let failed_assets = self.failed_assets.clone();

        tokio::spawn(async move {
            println!("[Worker {}] Started", worker_id);

            // Buffer for batch results (for potential batch DB updates in future)
            let mut results_buffer: Vec<ProcessingResult> = Vec::with_capacity(20);

            loop {
                // Check stop signal
                if stop_signal.load(Ordering::SeqCst) {
                    println!("[Worker {}] Stopped", worker_id);
                    break;
                }

                // Check pause signal
                if pause_signal.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                // Try to get work from queue (non-blocking)
                match work_rx.try_recv() {
                    Ok(asset) => {
                        // Process the asset
                        let result = process_asset(&asset, &db).await;

                        // Update counters
                        if result.success {
                            completed_assets.fetch_add(1, Ordering::SeqCst);
                        } else {
                            failed_assets.fetch_add(1, Ordering::SeqCst);
                            println!(
                                "[Worker {}] Failed to process asset {}: {:?}",
                                worker_id, asset.filename, result.error
                            );
                        }

                        results_buffer.push(result);

                        // Emit progress event every N assets processed
                        if results_buffer.len() >= EMIT_PROGRESS_EVERY_N_ASSETS {
                            results_buffer.clear();
                        }
                    }
                    Err(crossbeam::channel::TryRecvError::Empty) => {
                        // No work available, check if we're done
                        tokio::time::sleep(Duration::from_millis(50)).await;

                        // If queue is empty and we're not paused, we might be done
                        if work_rx.is_empty() && !pause_signal.load(Ordering::SeqCst) {
                            // Give a small grace period for more work to arrive
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            if work_rx.is_empty() {
                                println!("[Worker {}] No more work, exiting", worker_id);
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

            // Check if this is the last worker to exit
            // We'll mark as not running when all workers are joined
            println!("[Worker {}] Finished", worker_id);
        })
    }

    /// Spawn a task that periodically emits progress events
    fn spawn_progress_emitter(&self, app_handle: AppHandle) {
        let total_assets = self.total_assets.clone();
        let completed_assets = self.completed_assets.clone();
        let failed_assets = self.failed_assets.clone();
        let pause_signal = self.pause_signal.clone();
        let stop_signal = self.stop_signal.clone();
        let is_running = self.is_running.clone();
        let worker_handles = self.worker_handles.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(BATCH_UPDATE_INTERVAL_SEC));

            loop {
                ticker.tick().await;

                // Check if we should stop
                if stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                // Skip emitting progress if paused (workers aren't doing anything)
                if pause_signal.load(Ordering::SeqCst) {
                    continue;
                }

                // Emit progress event
                let progress = ProcessingProgress {
                    total: total_assets.load(Ordering::SeqCst),
                    completed: completed_assets.load(Ordering::SeqCst),
                    failed: failed_assets.load(Ordering::SeqCst),
                    is_paused: pause_signal.load(Ordering::SeqCst),
                    is_running: is_running.load(Ordering::SeqCst),
                };

                let _ = app_handle.emit("processing-progress", progress.clone());

                // Check if all work is done
                if progress.completed + progress.failed >= progress.total && progress.total > 0 {
                    println!(
                        "[Progress Emitter] All work done: {}/{} assets processed",
                        progress.completed, progress.total
                    );

                    // Wait for all workers to finish
                    let all_finished = {
                        let handles = worker_handles.read().await;
                        handles.iter().all(|handle| handle.is_finished())
                    };

                    if !all_finished {
                        // Still have active workers, wait and continue checking
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }

                    // All workers finished, mark as not running
                    is_running.store(false, Ordering::SeqCst);

                    // Emit final progress
                    let final_progress = ProcessingProgress {
                        total: total_assets.load(Ordering::SeqCst),
                        completed: completed_assets.load(Ordering::SeqCst),
                        failed: failed_assets.load(Ordering::SeqCst),
                        is_paused: false,
                        is_running: false,
                    };
                    let _ = app_handle.emit("processing-complete", final_progress);

                    break;
                }
            }
        });
    }

    /// Pause processing (workers will stop between assets)
    pub fn pause(&self) {
        self.pause_signal.store(true, Ordering::SeqCst);
    }

    /// Resume processing
    pub fn resume(&self) {
        self.pause_signal.store(false, Ordering::SeqCst);
    }

    /// Stop processing completely
    pub async fn stop(&self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.is_running.store(false, Ordering::SeqCst);

        // Wait for all workers to finish
        let handles = self.worker_handles.write().await;
        for handle in handles.iter() {
            handle.abort();
        }
    }

    /// Get current processing progress
    pub fn get_progress(&self) -> ProcessingProgress {
        ProcessingProgress {
            total: self.total_assets.load(Ordering::SeqCst),
            completed: self.completed_assets.load(Ordering::SeqCst),
            failed: self.failed_assets.load(Ordering::SeqCst),
            is_paused: self.pause_signal.load(Ordering::SeqCst),
            is_running: self.is_running.load(Ordering::SeqCst),
        }
    }

    /// Check if processing is currently running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Check if processing is paused
    pub fn is_paused(&self) -> bool {
        self.pause_signal.load(Ordering::SeqCst)
    }
}
