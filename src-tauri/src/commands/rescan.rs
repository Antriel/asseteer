use crate::commands::scan::{
    discover_files_streaming, insert_asset_row, load_search_excludes, populate_directories,
    DiscoveredAsset, ScanProgress, ScanProgressState,
};
use crate::utils::now_millis;
use crate::AppState;
use serde::Serialize;
use serde_json;
use sqlx;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

/// Minimum interval between progress event emissions
const EMIT_INTERVAL_MS: u128 = 100;

/// Asset identity key for diffing (matches the unique index on assets table)
#[derive(Hash, Eq, PartialEq)]
struct AssetKey {
    rel_path: String,
    zip_file_key: String, // COALESCE(zip_file, '')
    entry_key: String,    // COALESCE(zip_entry, filename)
}

impl AssetKey {
    fn from_discovered(asset: &DiscoveredAsset) -> Self {
        Self {
            rel_path: asset.rel_path.clone(),
            zip_file_key: asset.zip_file.clone().unwrap_or_default(),
            entry_key: asset
                .zip_entry
                .clone()
                .unwrap_or_else(|| asset.filename.clone()),
        }
    }

    fn from_existing(asset: &ExistingAsset) -> Self {
        Self {
            rel_path: asset.rel_path.clone(),
            zip_file_key: asset.zip_file.clone().unwrap_or_default(),
            entry_key: asset
                .zip_entry
                .clone()
                .unwrap_or_else(|| asset.filename.clone()),
        }
    }
}

/// Minimal asset row fetched from DB for comparison
#[derive(sqlx::FromRow)]
struct ExistingAsset {
    id: i64,
    rel_path: String,
    filename: String,
    zip_file: Option<String>,
    zip_entry: Option<String>,
    file_size: i64,
    fs_modified_at: Option<i64>,
}

/// Stored rescan preview (cached in AppState between preview and apply)
pub(crate) struct CachedRescanPreview {
    #[allow(dead_code)]
    pub folder_id: i64,
    pub added: Vec<DiscoveredAsset>,
    pub removed: Vec<i64>,                     // asset IDs to delete
    pub modified: Vec<(i64, DiscoveredAsset)>, // (asset_id, new disk data)
    pub unchanged_count: usize,
    pub warnings: Vec<String>,
    pub created_at: Instant,
}

/// Result returned to frontend from preview_rescan
#[derive(Clone, Serialize)]
pub struct RescanPreviewResult {
    pub preview_id: String,
    pub added_count: usize,
    pub removed_count: usize,
    pub modified_count: usize,
    pub unchanged_count: usize,
    pub warnings: Vec<String>,
}

/// Result returned to frontend from apply_rescan
#[derive(Clone, Serialize)]
pub struct RescanApplyResult {
    pub inserted: usize,
    pub deleted: usize,
    pub updated: usize,
}

/// Preview a rescan of a source folder. Discovers files on disk, compares against DB,
/// returns a diff summary. The full diff is cached so apply_rescan can commit it.
#[tauri::command]
pub async fn preview_rescan(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: i64,
) -> Result<RescanPreviewResult, String> {
    // 1. Get folder path
    let folder_path: String = sqlx::query_scalar("SELECT path FROM source_folders WHERE id = ?1")
        .bind(folder_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Folder with id {} not found", folder_id))?;

    let root_path_buf = PathBuf::from(&folder_path);
    if !root_path_buf.exists() {
        return Err(format!(
            "Folder path no longer exists on disk: {}",
            folder_path
        ));
    }

    // 2. Load search excludes for this folder
    let search_excludes = load_search_excludes(&state.pool, folder_id).await?;

    // 3. Discover files on disk (reuse streaming discovery, collect results)
    let (tx, mut rx) = mpsc::channel::<Vec<DiscoveredAsset>>(32);

    let progress = Arc::new(ScanProgressState {
        files_found: AtomicUsize::new(0),
        files_inserted: AtomicUsize::new(0),
        zips_scanned: AtomicUsize::new(0),
        discovery_complete: AtomicBool::new(false),
        warnings: std::sync::Mutex::new(Vec::new()),
    });

    let discover_app = app.clone();
    let discover_progress = progress.clone();
    let discovery_handle = tokio::task::spawn_blocking(move || {
        discover_files_streaming(
            &discover_app,
            &root_path_buf,
            folder_id,
            tx,
            &discover_progress,
            "rescan-progress",
            &search_excludes,
            None,
        )
    });

    // Collect all discovered assets
    let mut discovered: Vec<DiscoveredAsset> = Vec::new();
    let collect_app = app.clone();
    let collect_progress = progress.clone();
    let mut last_emit = Instant::now();

    while let Some(chunk) = rx.recv().await {
        discovered.extend(chunk);

        // Emit collection progress
        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let found = collect_progress.files_found.load(Ordering::Relaxed);
            let zips = collect_progress.zips_scanned.load(Ordering::Relaxed);
            let _ = collect_app.emit(
                "rescan-progress",
                ScanProgress {
                    phase: "scanning".to_string(),
                    files_found: found,
                    files_inserted: 0,
                    files_total: 0,
                    zips_scanned: zips,
                    current_path: None,
                    warnings: vec![],
                    folder_path: None,
                },
            );
            last_emit = Instant::now();
        }
    }

    discovery_handle
        .await
        .map_err(|e| format!("Discovery task panicked: {}", e))?
        .map_err(|e| e)?;

    let warnings: Vec<String> = progress
        .warnings
        .lock()
        .map(|mut w| w.drain(..).collect())
        .unwrap_or_default();

    // 3. Fetch existing assets from DB
    let existing: Vec<ExistingAsset> = sqlx::query_as(
        "SELECT id, rel_path, filename, zip_file, zip_entry, file_size, fs_modified_at
         FROM assets WHERE folder_id = ?1",
    )
    .bind(folder_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    // 4. Compute diff
    // Build lookup of existing assets by key
    let mut existing_map: HashMap<AssetKey, ExistingAsset> = HashMap::with_capacity(existing.len());
    for asset in existing {
        let key = AssetKey::from_existing(&asset);
        existing_map.insert(key, asset);
    }

    let mut added: Vec<DiscoveredAsset> = Vec::new();
    let mut modified: Vec<(i64, DiscoveredAsset)> = Vec::new();
    let mut unchanged_count: usize = 0;

    // Check each discovered asset against DB
    for disc in discovered {
        let key = AssetKey::from_discovered(&disc);
        if let Some(existing) = existing_map.remove(&key) {
            // Asset exists in DB — check if modified
            let size_changed = disc.file_size != existing.file_size;
            let mtime_changed = existing
                .fs_modified_at
                .map_or(true, |db_mtime| disc.fs_modified_at != db_mtime);

            if size_changed || mtime_changed {
                modified.push((existing.id, disc));
            } else {
                unchanged_count += 1;
            }
        } else {
            // Not in DB — new file
            added.push(disc);
        }
    }

    // Remaining in existing_map = removed from disk
    let removed: Vec<i64> = existing_map.into_values().map(|a| a.id).collect();

    let preview = CachedRescanPreview {
        folder_id,
        added,
        removed,
        modified,
        unchanged_count,
        warnings: warnings.clone(),
        created_at: Instant::now(),
    };

    let result = RescanPreviewResult {
        preview_id: format!("rescan-{}-{}", folder_id, now_millis()),
        added_count: preview.added.len(),
        removed_count: preview.removed.len(),
        modified_count: preview.modified.len(),
        unchanged_count: preview.unchanged_count,
        warnings,
    };

    // Emit completion
    let _ = app.emit(
        "rescan-progress",
        ScanProgress {
            phase: "preview-complete".to_string(),
            files_found: result.added_count + result.modified_count + result.unchanged_count,
            files_inserted: 0,
            files_total: result.added_count
                + result.removed_count
                + result.modified_count
                + result.unchanged_count,
            zips_scanned: progress.zips_scanned.load(Ordering::Relaxed),
            current_path: None,
            warnings: vec![],
            folder_path: None,
        },
    );

    // Cache the preview (one per folder, replaces any previous)
    state
        .rescan_previews
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?
        .insert(folder_id, preview);

    Ok(result)
}

/// Apply a previously previewed rescan. Inserts new assets, deletes removed assets,
/// and resets metadata for modified assets so they get reprocessed.
#[tauri::command]
pub async fn apply_rescan(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: i64,
) -> Result<RescanApplyResult, String> {
    // Retrieve and consume the cached preview
    let preview = state
        .rescan_previews
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?
        .remove(&folder_id)
        .ok_or_else(|| format!("No pending rescan preview for folder {}", folder_id))?;

    // Verify it's not stale (older than 30 minutes)
    if preview.created_at.elapsed().as_secs() > 1800 {
        return Err("Rescan preview expired (older than 30 minutes). Please preview again.".into());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let added_count = preview.added.len();
    let removed_count = preview.removed.len();
    let modified_count = preview.modified.len();
    let total_ops = added_count + removed_count + modified_count;

    // Emit initial progress
    let _ = app.emit(
        "rescan-progress",
        ScanProgress {
            phase: "applying".to_string(),
            files_found: total_ops,
            files_inserted: 0,
            files_total: total_ops,
            zips_scanned: 0,
            current_path: None,
            warnings: vec![],
            folder_path: None,
        },
    );

    // Apply in a transaction
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;
    let mut ops_done: usize = 0;
    let mut last_emit = Instant::now();

    // Delete removed assets (CASCADE handles metadata cleanup)
    for asset_id in &preview.removed {
        sqlx::query("DELETE FROM assets WHERE id = ?1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        ops_done += 1;
        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let _ = app.emit(
                "rescan-progress",
                ScanProgress {
                    phase: "applying".to_string(),
                    files_found: total_ops,
                    files_inserted: ops_done,
                    files_total: total_ops,
                    zips_scanned: 0,
                    current_path: None,
                    warnings: vec![],
                    folder_path: None,
                },
            );
            last_emit = Instant::now();
        }
    }

    // Update modified assets: update file stats, delete derived metadata
    for (asset_id, disc) in &preview.modified {
        // Update the asset row (including searchable_path to refresh FTS triggers)
        sqlx::query(
            "UPDATE assets SET file_size = ?1, fs_modified_at = ?2, modified_at = ?3, searchable_path = ?4, zip_compression = ?5 WHERE id = ?6",
        )
        .bind(disc.file_size)
        .bind(disc.fs_modified_at)
        .bind(now)
        .bind(&disc.searchable_path)
        .bind(&disc.zip_compression)
        .bind(asset_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        // Delete derived metadata so processing pipeline re-processes
        sqlx::query("DELETE FROM image_metadata WHERE asset_id = ?1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM audio_metadata WHERE asset_id = ?1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM audio_embeddings WHERE asset_id = ?1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM processing_errors WHERE asset_id = ?1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        ops_done += 1;
        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let _ = app.emit(
                "rescan-progress",
                ScanProgress {
                    phase: "applying".to_string(),
                    files_found: total_ops,
                    files_inserted: ops_done,
                    files_total: total_ops,
                    zips_scanned: 0,
                    current_path: None,
                    warnings: vec![],
                    folder_path: None,
                },
            );
            last_emit = Instant::now();
        }
    }

    // Insert new assets (FTS INSERT trigger removed — populate explicitly below)
    let max_id_before_inserts: i64 = if !preview.added.is_empty() {
        sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM assets")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
    } else {
        0
    };

    for asset in &preview.added {
        insert_asset_row(&mut tx, asset, now).await?;

        ops_done += 1;
        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let _ = app.emit(
                "rescan-progress",
                ScanProgress {
                    phase: "applying".to_string(),
                    files_found: total_ops,
                    files_inserted: ops_done,
                    files_total: total_ops,
                    zips_scanned: 0,
                    current_path: None,
                    warnings: vec![],
                    folder_path: None,
                },
            );
            last_emit = Instant::now();
        }
    }

    // Populate FTS for newly inserted assets
    if !preview.added.is_empty() {
        sqlx::query(
            "INSERT INTO assets_fts_sub(rowid, filename, searchable_path)
             SELECT id, filename, searchable_path FROM assets
             WHERE folder_id = ?1 AND id > ?2",
        )
        .bind(folder_id)
        .bind(max_id_before_inserts)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO assets_fts_word(rowid, filename, searchable_path)
             SELECT id, filename, searchable_path FROM assets
             WHERE folder_id = ?1 AND id > ?2",
        )
        .bind(folder_id)
        .bind(max_id_before_inserts)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    // Rebuild precomputed directory tree (non-fatal — don't block rescan completion)
    if let Err(e) = populate_directories(&state.pool, &state.db_path, folder_id).await {
        eprintln!(
            "[Rescan] Failed to populate directories for folder {}: {}",
            folder_id, e
        );
    }

    // Encode warnings as JSON (NULL if empty)
    let warnings_json: Option<String> = if preview.warnings.is_empty() {
        None
    } else {
        serde_json::to_string(&preview.warnings).ok()
    };

    // Update source folder stats and persist warnings
    sqlx::query(
        "UPDATE source_folders SET last_scanned_at = ?1, asset_count = (SELECT COUNT(*) FROM assets WHERE folder_id = ?2), scan_warnings = ?3 WHERE id = ?2",
    )
    .bind(now)
    .bind(folder_id)
    .bind(warnings_json)
    .execute(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    // Emit completion
    let _ = app.emit(
        "rescan-progress",
        ScanProgress {
            phase: "complete".to_string(),
            files_found: total_ops,
            files_inserted: ops_done,
            files_total: total_ops,
            zips_scanned: 0,
            current_path: None,
            warnings: vec![],
            folder_path: None,
        },
    );

    Ok(RescanApplyResult {
        inserted: added_count,
        deleted: removed_count,
        updated: modified_count,
    })
}
