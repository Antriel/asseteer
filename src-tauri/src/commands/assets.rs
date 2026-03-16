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

    // Find which assets are missing thumbnails
    let placeholders: Vec<String> = asset_ids.iter().map(|_| "?".to_string()).collect();
    let query = format!(
        "SELECT a.id, a.file_size FROM assets a
         LEFT JOIN image_metadata im ON a.id = im.asset_id
         WHERE a.id IN ({}) AND a.asset_type = 'image'
         AND (im.asset_id IS NULL OR im.thumbnail_data IS NULL)",
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

    // Generate thumbnails in parallel using blocking threads
    let pool = state.pool.clone();
    let mut generated = Vec::new();

    for asset in assets {
        match generate_thumbnail_for_asset(&asset).await {
            Ok((width, height, thumbnail_data)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Upsert: insert or update thumbnail_data if it was NULL
                let result = sqlx::query(
                    "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT (asset_id) DO UPDATE SET
                         thumbnail_data = excluded.thumbnail_data
                     WHERE image_metadata.thumbnail_data IS NULL",
                )
                .bind(asset.id)
                .bind(width)
                .bind(height)
                .bind(&thumbnail_data)
                .bind(now)
                .execute(&pool)
                .await;

                if result.is_ok() {
                    generated.push(asset.id);
                }
            }
            Err(e) => {
                eprintln!("Failed to generate thumbnail for asset {}: {}", asset.id, e);
            }
        }
    }

    Ok(generated)
}
