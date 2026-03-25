use crate::commands::scan::{compute_searchable_path, load_search_excludes};
use crate::models::SourceFolder;
use crate::{database, AppState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, State};

/// List all source folders
#[tauri::command]
pub async fn list_folders(
    state: State<'_, AppState>,
) -> Result<Vec<SourceFolder>, String> {
    sqlx::query_as::<_, SourceFolder>(
        "SELECT id, path, label, added_at, last_scanned_at, asset_count, status, scan_warnings
         FROM source_folders
         ORDER BY label COLLATE NOCASE"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to list folders: {}", e))
}

/// Progress event payload for folder removal
#[derive(Clone, Serialize)]
pub struct FolderRemoveProgress {
    pub phase: String,
    pub deleted: i64,
    pub total: i64,
}

const REMOVE_BATCH_SIZE: i64 = 5000;

/// Remove a source folder: batch-delete assets with progress, then remove
/// the folder row, checkpoint WAL, and VACUUM to reclaim disk space.
#[tauri::command]
pub async fn remove_folder(
    app: AppHandle,
    folder_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Get total asset count for progress reporting
    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM assets WHERE folder_id = ?")
            .bind(folder_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| format!("Failed to count assets: {}", e))?;

    let emit_progress = |phase: &str, deleted: i64| {
        let _ = app.emit(
            "folder-remove-progress",
            FolderRemoveProgress {
                phase: phase.to_string(),
                deleted,
                total,
            },
        );
    };

    // Batch-delete assets to avoid a single huge CASCADE transaction
    let mut deleted: i64 = 0;
    emit_progress("deleting", 0);

    loop {
        let result = sqlx::query(
            "DELETE FROM assets WHERE id IN (SELECT id FROM assets WHERE folder_id = ? LIMIT ?)",
        )
        .bind(folder_id)
        .bind(REMOVE_BATCH_SIZE)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to delete assets: {}", e))?;

        let rows = result.rows_affected() as i64;
        if rows == 0 {
            break;
        }
        deleted += rows;
        emit_progress("deleting", deleted);
    }

    // Delete the folder row (remaining CASCADE handles scan_sessions, excludes)
    let result = sqlx::query("DELETE FROM source_folders WHERE id = ?")
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to remove folder: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Folder with id {} not found", folder_id));
    }

    // Checkpoint WAL to flush deletion pages and reclaim WAL space.
    // No VACUUM — it rewrites the entire DB into the WAL, which is slow
    // and leaves a massive WAL behind with auto-checkpoint disabled.
    // SQLite reuses free pages internally on future inserts.
    emit_progress("compacting", deleted);

    if let Err(e) = database::checkpoint_truncate(&state.pool).await {
        eprintln!("[DB] WAL checkpoint after folder removal failed: {}", e);
    }

    emit_progress("done", deleted);

    Ok(())
}

/// Rename a source folder's label
#[tauri::command]
pub async fn rename_folder(
    folder_id: i64,
    label: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let result = sqlx::query("UPDATE source_folders SET label = ? WHERE id = ?")
        .bind(&label)
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to rename folder: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Folder with id {} not found", folder_id));
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SearchExclude {
    pub zip_file: Option<String>,
    pub excluded_path: String,
}

/// Update search excludes for a folder and re-index all its assets' searchable_path
#[tauri::command]
pub async fn update_search_excludes(
    folder_id: i64,
    excludes: Vec<SearchExclude>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Replace all excludes for this folder
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM folder_search_excludes WHERE source_folder_id = ?1")
        .bind(folder_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for entry in &excludes {
        sqlx::query(
            "INSERT INTO folder_search_excludes (source_folder_id, zip_file, excluded_path)
             VALUES (?1, ?2, ?3)",
        )
        .bind(folder_id)
        .bind(&entry.zip_file)
        .bind(&entry.excluded_path)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    // Re-index: load the new excludes, fetch all assets, recompute searchable_path
    let search_excludes = load_search_excludes(&state.pool, folder_id).await?;

    let assets: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, rel_path, zip_file, zip_entry FROM assets WHERE folder_id = ?1",
    )
    .bind(folder_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    // Update in batches within a transaction
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;
    for (id, rel_path, zip_file, zip_entry) in &assets {
        let sp = compute_searchable_path(rel_path, zip_file.as_deref(), zip_entry.as_deref(), &search_excludes);
        sqlx::query("UPDATE assets SET searchable_path = ?1 WHERE id = ?2")
            .bind(&sp)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ZipDirGroup {
    pub rel_path: String,
    pub zip_file: String,
    pub dirs: Vec<String>,
}

/// Get directory trees inside all ZIP files for a folder.
/// Uses a partial covering index (idx_assets_zip_tree) for an index-only scan,
/// extracts unique directory prefixes in Rust, and returns compact grouped results.
#[tauri::command]
pub async fn get_zip_dir_trees(
    folder_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<ZipDirGroup>, String> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT rel_path, zip_file, zip_entry FROM assets
         WHERE folder_id = ?1 AND zip_file IS NOT NULL AND zip_entry IS NOT NULL
         ORDER BY rel_path, zip_file",
    )
    .bind(folder_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to load zip entries: {}", e))?;

    // Rows arrive sorted by (rel_path, zip_file) from the covering index.
    // Process sequentially: collect only unique leaf directories per zip.
    // Leaf dirs are sufficient — JS buildTree() creates intermediate nodes.
    let mut result: Vec<ZipDirGroup> = Vec::new();
    let mut current_rp = String::new();
    let mut current_zf = String::new();
    let mut dirs = HashSet::<String>::new();

    for (rel_path, zip_file, zip_entry) in rows {
        if rel_path != current_rp || zip_file != current_zf {
            if !dirs.is_empty() {
                let mut sorted: Vec<String> = dirs.drain().collect();
                sorted.sort_unstable();
                result.push(ZipDirGroup {
                    rel_path: std::mem::take(&mut current_rp),
                    zip_file: std::mem::take(&mut current_zf),
                    dirs: sorted,
                });
            }
            current_rp = rel_path;
            current_zf = zip_file;
        }
        // Extract leaf directory only (everything before last '/')
        if let Some(last_slash) = zip_entry.rfind('/') {
            let dir_part = &zip_entry[..last_slash];
            // contains() borrows &str — no allocation for the 99%+ duplicate case
            if !dirs.contains(dir_part) {
                dirs.insert(dir_part.to_string());
            }
        }
    }
    if !dirs.is_empty() {
        let mut sorted: Vec<String> = dirs.drain().collect();
        sorted.sort_unstable();
        result.push(ZipDirGroup {
            rel_path: current_rp,
            zip_file: current_zf,
            dirs: sorted,
        });
    }

    Ok(result)
}
