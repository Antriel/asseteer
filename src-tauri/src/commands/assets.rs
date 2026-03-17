use crate::utils::load_asset_bytes;
use crate::AppState;
use crate::models::Asset;
use tauri::State;

/// Get raw bytes for an asset (works for both regular files and zip entries)
#[tauri::command]
pub async fn get_asset_bytes(
    state: State<'_, AppState>,
    asset_id: i64,
) -> Result<Vec<u8>, String> {
    // Load asset from database
    let asset = sqlx::query_as::<_, Asset>("SELECT * FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| format!("Failed to load asset: {}", e))?;

    // Load bytes (handles both regular files and zip entries)
    load_asset_bytes(&asset)
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
