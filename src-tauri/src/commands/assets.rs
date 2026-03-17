use crate::models::Asset;
use crate::task_system::processor::generate_thumbnail_for_asset;
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

/// Ensure thumbnails exist for a batch of asset IDs.
/// Generates missing thumbnails on demand and stores them in the database.
/// Returns the list of asset IDs that were successfully generated.
#[tauri::command]
pub async fn ensure_thumbnails(
    state: State<'_, AppState>,
    asset_ids: Vec<i64>,
) -> Result<Vec<i64>, String> {
    if asset_ids.is_empty() {
        return Ok(vec![]);
    }

    // Find which assets are missing thumbnails.
    // Skip small images (both dimensions <= 128px) — the frontend shows originals directly.
    let placeholders: Vec<String> = asset_ids.iter().map(|_| "?".to_string()).collect();
    let query = format!(
        "SELECT a.id, a.file_size FROM assets a
         LEFT JOIN image_metadata im ON a.id = im.asset_id
         WHERE a.id IN ({}) AND a.asset_type = 'image'
         AND (im.asset_id IS NULL OR im.thumbnail_data IS NULL)
         AND NOT (im.width IS NOT NULL AND im.width <= 128 AND im.height IS NOT NULL AND im.height <= 128)",
        placeholders.join(",")
    );

    let mut q = sqlx::query_as::<_, (i64, i64)>(&query);
    for id in &asset_ids {
        q = q.bind(id);
    }

    let missing: Vec<(i64, i64)> = q
        .fetch_all(&state.pool)
        .await
        .map_err(|e| format!("Failed to query missing thumbnails: {}", e))?;

    if missing.is_empty() {
        return Ok(vec![]);
    }

    // Load assets that need thumbnails
    let missing_ids: Vec<i64> = missing.iter().map(|(id, _)| *id).collect();
    let placeholders: Vec<String> = missing_ids.iter().map(|_| "?".to_string()).collect();
    let assets_query = format!(
        "SELECT * FROM assets WHERE id IN ({})",
        placeholders.join(",")
    );

    let mut q = sqlx::query_as::<_, Asset>(&assets_query);
    for id in &missing_ids {
        q = q.bind(id);
    }

    let assets: Vec<Asset> = q
        .fetch_all(&state.pool)
        .await
        .map_err(|e| format!("Failed to load assets: {}", e))?;

    // Generate thumbnails sequentially to limit memory usage.
    // Each image decode can use ~150MB (decoded RGBA + resize buffers),
    // so parallel decoding quickly exhausts memory on large images.
    let mut results: Vec<(i64, i32, i32, Vec<u8>)> = Vec::new();

    for asset in assets {
        let id = asset.id;
        match generate_thumbnail_for_asset(&asset).await {
            Ok((width, height, data)) => results.push((id, width, height, data)),
            Err(e) => eprintln!("Failed to generate thumbnail: Asset {}: {}", id, e),
        }
    }

    // Write results to DB
    let pool = state.pool.clone();
    let mut generated = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for (asset_id, width, height, thumbnail_data) in results {
        let db_result = sqlx::query(
            "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (asset_id) DO UPDATE SET
                 thumbnail_data = excluded.thumbnail_data
             WHERE image_metadata.thumbnail_data IS NULL",
        )
        .bind(asset_id)
        .bind(width)
        .bind(height)
        .bind(&thumbnail_data)
        .bind(now)
        .execute(&pool)
        .await;

        if db_result.is_ok() {
            generated.push(asset_id);
        }
    }

    Ok(generated)
}
