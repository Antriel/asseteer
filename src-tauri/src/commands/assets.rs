use crate::models::Asset;
use crate::utils::load_asset_bytes;
use crate::AppState;
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
