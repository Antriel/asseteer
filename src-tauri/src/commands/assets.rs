use crate::zip_cache;
use crate::AppState;
use crate::models::Asset;
use tauri::ipc::Response;
use tauri::State;

/// Get raw bytes for an asset (works for both regular files and zip entries).
/// Returns a binary IPC response (ArrayBuffer on the JS side) to avoid JSON number[] overhead.
#[tauri::command]
pub async fn get_asset_bytes(
    state: State<'_, AppState>,
    asset_id: i64,
) -> Result<Response, String> {
    // Load asset from database with folder_path from source_folders JOIN
    let asset = sqlx::query_as::<_, Asset>(
        "SELECT a.*, sf.path as folder_path
         FROM assets a
         JOIN source_folders sf ON a.folder_id = sf.id
         WHERE a.id = ?"
    )
    .bind(asset_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| format!("Failed to load asset: {}", e))?;

    // Load bytes — uses the nested-ZIP memory cache for nested ZIP assets.
    // Wrap in Response so Tauri sends raw bytes instead of JSON number[].
    zip_cache::load_asset_bytes_cached(&asset).map(Response::new)
}

/// Request thumbnail generation for a batch of asset IDs.
/// Pushes into the background worker's channel (non-blocking).
#[tauri::command]
pub async fn request_thumbnails(
    state: State<'_, AppState>,
    asset_ids: Vec<i64>,
) -> Result<(), String> {
    if !asset_ids.is_empty() {
        state.thumbnail_worker.request(asset_ids);
    }
    Ok(())
}

/// Cancel pending thumbnail requests (assets scrolled out of view).
#[tauri::command]
pub async fn cancel_thumbnails(
    state: State<'_, AppState>,
    asset_ids: Vec<i64>,
) -> Result<(), String> {
    if !asset_ids.is_empty() {
        state.thumbnail_worker.cancel(asset_ids);
    }
    Ok(())
}
