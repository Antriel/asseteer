use crate::models::SourceFolder;
use crate::AppState;
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
