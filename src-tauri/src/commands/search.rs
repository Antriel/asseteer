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
    let mut sql = String::from(
        "SELECT
            id, filename, path, zip_entry, asset_type, format, file_size,
            width, height, duration_ms, sample_rate, channels,
            created_at, modified_at, processing_status, processing_error
        FROM assets"
    );

    let mut conditions = Vec::new();
    let mut where_added = false;

    // Full-text search
    let search_text = if let Some(text) = &query.text {
        if !text.is_empty() {
            sql.push_str(" INNER JOIN assets_fts fts ON assets.id = fts.rowid");
            conditions.push("fts MATCH ?");
            where_added = true;
            Some(format!("{}*", text.trim()))
        } else {
            None
        }
    } else {
        None
    };

    // Asset type filter
    if query.asset_type.is_some() {
        conditions.push("asset_type = ?");
    }

    // Build WHERE clause
    if !conditions.is_empty() {
        if !where_added {
            sql.push_str(" WHERE ");
        } else {
            sql.push_str(" WHERE ");
        }
        sql.push_str(&conditions.join(" AND "));
    }

    // Sorting and pagination
    sql.push_str(" ORDER BY filename COLLATE NOCASE ASC");
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit, query.offset));

    // Execute query with dynamic binding
    let mut query_builder = sqlx::query_as::<_, Asset>(&sql);

    if let Some(text) = search_text {
        query_builder = query_builder.bind(text);
    }

    if let Some(asset_type) = &query.asset_type {
        query_builder = query_builder.bind(asset_type);
    }

    let assets = query_builder
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

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
