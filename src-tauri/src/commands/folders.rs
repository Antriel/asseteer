use crate::commands::scan::{compute_searchable_path, load_search_excludes};
use crate::models::SourceFolder;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

/// List all source folders
#[tauri::command]
pub async fn list_folders(
    state: State<'_, AppState>,
) -> Result<Vec<SourceFolder>, String> {
    sqlx::query_as::<_, SourceFolder>(
        "SELECT id, path, label, added_at, last_scanned_at, asset_count, status
         FROM source_folders
         ORDER BY label COLLATE NOCASE"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to list folders: {}", e))
}

/// Remove a source folder (CASCADE deletes all its assets + metadata)
#[tauri::command]
pub async fn remove_folder(
    folder_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM source_folders WHERE id = ?")
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to remove folder: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Folder with id {} not found", folder_id));
    }

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
