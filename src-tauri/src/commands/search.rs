use crate::{models::Asset, AppState};
use serde::Deserialize;
use sqlx;
use tauri::State;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub asset_type: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// Search for assets
#[tauri::command]
pub async fn search_assets(
    state: State<'_, AppState>,
    query: SearchQuery,
) -> Result<Vec<Asset>, String> {
    // Check if we need full-text search
    let search_text = query.text.as_ref()
        .filter(|t| !t.is_empty())
        .map(|t| format!("{}*", t.trim()));

    let assets = if let Some(text) = search_text {
        // Full-text search query
        search_with_fts(&state, text, query.asset_type.as_deref(), query.limit, query.offset).await?
    } else {
        // Simple query without FTS
        search_without_fts(&state, query.asset_type.as_deref(), query.limit, query.offset).await?
    };

    Ok(assets)
}

/// Search using full-text search
async fn search_with_fts(
    state: &AppState,
    search_text: String,
    asset_type: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<Asset>, String> {
    let assets = if let Some(atype) = asset_type {
        sqlx::query_as::<_, Asset>(
            "SELECT
                assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
                assets.format, assets.file_size, assets.created_at, assets.modified_at,
                image_metadata.width, image_metadata.height,
                audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
            FROM assets
            INNER JOIN assets_fts ON assets.id = assets_fts.rowid
            LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
            LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
            WHERE assets_fts MATCH ? AND assets.asset_type = ?
            ORDER BY assets.filename COLLATE NOCASE ASC
            LIMIT ? OFFSET ?"
        )
        .bind(search_text)
        .bind(atype)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<_, Asset>(
            "SELECT
                assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
                assets.format, assets.file_size, assets.created_at, assets.modified_at,
                image_metadata.width, image_metadata.height,
                audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
            FROM assets
            INNER JOIN assets_fts ON assets.id = assets_fts.rowid
            LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
            LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
            WHERE assets_fts MATCH ?
            ORDER BY assets.filename COLLATE NOCASE ASC
            LIMIT ? OFFSET ?"
        )
        .bind(search_text)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(assets)
}

/// Search without full-text search
async fn search_without_fts(
    state: &AppState,
    asset_type: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<Asset>, String> {
    let assets = if let Some(atype) = asset_type {
        sqlx::query_as::<_, Asset>(
            "SELECT
                assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
                assets.format, assets.file_size, assets.created_at, assets.modified_at,
                image_metadata.width, image_metadata.height,
                audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
            FROM assets
            LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
            LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
            WHERE asset_type = ?
            ORDER BY filename COLLATE NOCASE ASC
            LIMIT ? OFFSET ?"
        )
        .bind(atype)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<_, Asset>(
            "SELECT
                assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
                assets.format, assets.file_size, assets.created_at, assets.modified_at,
                image_metadata.width, image_metadata.height,
                audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
            FROM assets
            LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
            LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
            ORDER BY filename COLLATE NOCASE ASC
            LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(assets)
}

/// Get asset count
#[tauri::command]
pub async fn get_asset_count(state: State<'_, AppState>) -> Result<i64, String> {
    let result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}
