//! Dedicated background thumbnail worker.
//!
//! The frontend sends request/cancel messages via Tauri commands.
//! A single background task drains a channel, generates thumbnails
//! (parallel for filesystem, sequential per ZIP), and emits
//! `thumbnail-ready` / `thumbnail-stats` events.

use crate::models::Asset;
use crate::task_system::processor::generate_thumbnail_for_asset;
use crate::utils::{resolve_zip_path, now_millis};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Messages the frontend can send to the thumbnail worker.
#[derive(Debug)]
pub enum ThumbnailMsg {
    Request(Vec<i64>),
    Cancel(Vec<i64>),
}

/// Emitted per-thumbnail when generation completes.
#[derive(Clone, Serialize)]
pub struct ThumbnailReady {
    pub asset_id: i64,
    pub success: bool,
}

/// Periodic stats emitted to the frontend.
#[derive(Clone, Serialize)]
pub struct ThumbnailStats {
    pub queued: usize,
    pub processing: usize,
    pub loaded: u64,
    pub failed: u64,
    pub rate: f64,
}

/// Handle for sending messages to the worker from Tauri commands.
pub struct ThumbnailWorkerHandle {
    tx: mpsc::UnboundedSender<ThumbnailMsg>,
}

impl ThumbnailWorkerHandle {
    pub fn request(&self, ids: Vec<i64>) {
        let _ = self.tx.send(ThumbnailMsg::Request(ids));
    }

    pub fn cancel(&self, ids: Vec<i64>) {
        let _ = self.tx.send(ThumbnailMsg::Cancel(ids));
    }
}

// ---------------------------------------------------------------------------
// Worker setup
// ---------------------------------------------------------------------------

/// Max filesystem images decoded concurrently.
const MAX_CONCURRENT_FS: usize = 3;

/// Start the thumbnail worker. Call once during app setup.
pub fn start_worker(app: &AppHandle, pool: SqlitePool) -> ThumbnailWorkerHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(worker_loop(rx, app_handle, pool));
    ThumbnailWorkerHandle { tx }
}

// ---------------------------------------------------------------------------
// Internal state (single-owner, no Mutex needed — lives only in worker_loop)
// ---------------------------------------------------------------------------

struct WorkerState {
    /// LIFO stack — most recently requested at the top.
    pending: Vec<i64>,
    /// IDs the frontend no longer cares about.
    cancelled: HashSet<i64>,
    /// Dedup: IDs currently in `pending` or being processed.
    in_flight: HashSet<i64>,
    // Stats
    loaded: u64,
    failed: u64,
    processing: usize,
    recent_timestamps: Vec<u64>,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            cancelled: HashSet::new(),
            in_flight: HashSet::new(),
            loaded: 0,
            failed: 0,
            processing: 0,
            recent_timestamps: Vec::new(),
        }
    }

    fn add_requests(&mut self, ids: Vec<i64>) {
        for id in ids {
            self.cancelled.remove(&id);
            if self.in_flight.insert(id) {
                self.pending.push(id);
            }
        }
    }

    fn add_cancels(&mut self, ids: Vec<i64>) {
        for id in ids {
            self.cancelled.insert(id);
            self.in_flight.remove(&id);
        }
    }

    /// Pop up to `n` non-cancelled IDs from the LIFO stack.
    fn pop_batch(&mut self, n: usize) -> Vec<i64> {
        let mut batch = Vec::with_capacity(n);
        while batch.len() < n {
            match self.pending.pop() {
                Some(id) => {
                    if self.cancelled.remove(&id) {
                        self.in_flight.remove(&id);
                        continue;
                    }
                    batch.push(id);
                }
                None => break,
            }
        }
        // Prevent unbounded growth of cancelled set
        if self.cancelled.len() > 500 {
            self.cancelled.retain(|id| self.in_flight.contains(id));
        }
        batch
    }

    fn is_cancelled(&self, id: i64) -> bool {
        self.cancelled.contains(&id)
    }

    fn record_success(&mut self, id: i64) {
        self.in_flight.remove(&id);
        self.loaded += 1;
        self.recent_timestamps.push(now_millis());
    }

    fn record_failure(&mut self, id: i64) {
        self.in_flight.remove(&id);
        self.failed += 1;
    }

    fn stats(&self) -> ThumbnailStats {
        let now = now_millis();
        let cutoff = now.saturating_sub(5000);
        let recent = self.recent_timestamps.iter().filter(|&&t| t > cutoff).count();
        ThumbnailStats {
            queued: self.pending.len(),
            processing: self.processing,
            loaded: self.loaded,
            failed: self.failed,
            rate: recent as f64 / 5.0,
        }
    }

    fn trim_timestamps(&mut self) {
        let cutoff = now_millis().saturating_sub(10_000);
        self.recent_timestamps.retain(|&t| t > cutoff);
    }
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------

async fn worker_loop(
    mut rx: mpsc::UnboundedReceiver<ThumbnailMsg>,
    app: AppHandle,
    pool: SqlitePool,
) {
    let mut state = WorkerState::new();

    // Periodic stats timer
    let mut stats_interval = tokio::time::interval(std::time::Duration::from_millis(500));
    stats_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // If we have pending work, don't block — use select to also process
        // incoming messages and emit stats while working.
        if state.pending.is_empty() {
            // Nothing to do: wait for a message or stats tick
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(m) => apply_msg(&mut state, m),
                        None => break, // channel closed
                    }
                }
                _ = stats_interval.tick() => {
                    let _ = app.emit("thumbnail-stats", state.stats());
                }
            }
            continue;
        }

        // We have work — drain all pending messages first (non-blocking)
        drain_messages(&mut rx, &mut state);

        // Emit stats
        let _ = app.emit("thumbnail-stats", state.stats());

        // Pop a batch
        let batch = state.pop_batch(MAX_CONCURRENT_FS * 2);
        if batch.is_empty() {
            continue;
        }

        // Find which need generation
        let missing_set: HashSet<i64> = find_missing_thumbnails(&pool, &batch)
            .await
            .into_iter()
            .collect();

        // Emit ready for those that already have thumbnails
        for &id in &batch {
            if !missing_set.contains(&id) {
                state.record_success(id);
                let _ = app.emit("thumbnail-ready", ThumbnailReady { asset_id: id, success: true });
            }
        }

        let missing_ids: Vec<i64> = batch.into_iter().filter(|id| missing_set.contains(id)).collect();
        if missing_ids.is_empty() {
            continue;
        }

        // Load asset records (with folder_path from JOIN)
        let assets = load_assets(&pool, &missing_ids).await;

        // Separate filesystem vs ZIP assets
        // Group ZIP assets by resolved zip path (folder_path + rel_path + zip_file)
        let mut zip_groups: HashMap<String, Vec<Asset>> = HashMap::new();
        let mut fs_assets: Vec<Asset> = Vec::new();

        for asset in assets {
            if asset.zip_entry.is_some() {
                let zip_key = resolve_zip_path(&asset);
                zip_groups.entry(zip_key).or_default().push(asset);
            } else {
                fs_assets.push(asset);
            }
        }

        state.processing = fs_assets.len() + zip_groups.values().map(|v| v.len()).sum::<usize>();

        // Process filesystem images concurrently with semaphore
        let sem = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_FS));
        let mut handles = Vec::new();

        for asset in fs_assets {
            let pool_c = pool.clone();
            let sem_c = sem.clone();
            let app_c = app.clone();

            handles.push(tauri::async_runtime::spawn(async move {
                let _permit = sem_c.acquire().await.unwrap();
                let success = process_single_thumbnail(&asset, &pool_c).await;
                let _ = app_c.emit("thumbnail-ready", ThumbnailReady { asset_id: asset.id, success });
                (asset.id, success)
            }));
        }

        // Process ZIP groups sequentially (one entry at a time to limit memory)
        for (_zip_path, zip_assets) in &zip_groups {
            for asset in zip_assets {
                if state.is_cancelled(asset.id) {
                    state.in_flight.remove(&asset.id);
                    state.processing = state.processing.saturating_sub(1);
                    continue;
                }

                // Drain messages between ZIP entries to pick up cancellations
                drain_messages(&mut rx, &mut state);

                let success = process_single_thumbnail(asset, &pool).await;
                if success {
                    state.record_success(asset.id);
                } else {
                    state.record_failure(asset.id);
                }
                state.processing = state.processing.saturating_sub(1);
                let _ = app.emit("thumbnail-ready", ThumbnailReady { asset_id: asset.id, success });
            }
        }

        // Collect filesystem results
        for h in handles {
            if let Ok((id, success)) = h.await {
                if success {
                    state.record_success(id);
                } else {
                    state.record_failure(id);
                }
            }
            state.processing = state.processing.saturating_sub(1);
        }

        state.trim_timestamps();
    }
}

fn apply_msg(state: &mut WorkerState, msg: ThumbnailMsg) {
    match msg {
        ThumbnailMsg::Request(ids) => state.add_requests(ids),
        ThumbnailMsg::Cancel(ids) => state.add_cancels(ids),
    }
}

fn drain_messages(rx: &mut mpsc::UnboundedReceiver<ThumbnailMsg>, state: &mut WorkerState) {
    while let Ok(msg) = rx.try_recv() {
        apply_msg(state, msg);
    }
}

// ---------------------------------------------------------------------------
// Thumbnail generation
// ---------------------------------------------------------------------------

async fn process_single_thumbnail(asset: &Asset, pool: &SqlitePool) -> bool {
    match generate_thumbnail_for_asset(asset).await {
        Ok((width, height, data)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            sqlx::query(
                "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     thumbnail_data = excluded.thumbnail_data
                 WHERE image_metadata.thumbnail_data IS NULL",
            )
            .bind(asset.id)
            .bind(width)
            .bind(height)
            .bind(&data)
            .bind(now)
            .execute(pool)
            .await
            .is_ok()
        }
        Err(e) => {
            eprintln!("Thumbnail failed for asset {}: {}", asset.id, e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

async fn find_missing_thumbnails(pool: &SqlitePool, ids: &[i64]) -> Vec<i64> {
    if ids.is_empty() {
        return vec![];
    }
    let mut result = Vec::new();
    for chunk in ids.chunks(999) {
        let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "SELECT a.id FROM assets a
             LEFT JOIN image_metadata im ON a.id = im.asset_id
             WHERE a.id IN ({}) AND a.asset_type = 'image'
             AND (im.asset_id IS NULL OR im.thumbnail_data IS NULL)
             AND NOT (im.width IS NOT NULL AND im.width <= 128 AND im.height IS NOT NULL AND im.height <= 128)
             AND NOT (a.zip_entry IS NOT NULL AND a.zip_compression = 'store' AND a.zip_entry NOT LIKE '%.zip/%')",
            placeholders.join(",")
        );
        let mut q = sqlx::query_as::<_, (i64,)>(&query);
        for id in chunk {
            q = q.bind(id);
        }
        match q.fetch_all(pool).await {
            Ok(rows) => result.extend(rows.into_iter().map(|(id,)| id)),
            Err(e) => {
                eprintln!("Failed to query missing thumbnails: {}", e);
            }
        }
    }
    result
}

async fn load_assets(pool: &SqlitePool, ids: &[i64]) -> Vec<Asset> {
    if ids.is_empty() {
        return vec![];
    }
    let mut result = Vec::new();
    for chunk in ids.chunks(999) {
        let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "SELECT a.*, sf.path as folder_path FROM assets a
             JOIN source_folders sf ON a.folder_id = sf.id
             WHERE a.id IN ({})",
            placeholders.join(",")
        );
        let mut q = sqlx::query_as::<_, Asset>(&query);
        for id in chunk {
            q = q.bind(id);
        }
        match q.fetch_all(pool).await {
            Ok(assets) => result.extend(assets),
            Err(e) => {
                eprintln!("Failed to load assets: {}", e);
            }
        }
    }
    result
}

