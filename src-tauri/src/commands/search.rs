use crate::{models::Asset, AppState};
use serde::Deserialize;
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
    let conn = state.db.lock().map_err(|e| e.to_string())?;

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
    if let Some(text) = &query.text {
        if !text.is_empty() {
            sql.push_str(" INNER JOIN assets_fts fts ON assets.id = fts.rowid");
            conditions.push("fts MATCH ?");
            where_added = true;
        }
    }

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

    // Execute query
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(text) = &query.text {
        if !text.is_empty() {
            params_vec.push(Box::new(format!("{}*", text.trim())));
        }
    }

    if let Some(asset_type) = &query.asset_type {
        params_vec.push(Box::new(asset_type.clone()));
    }

    let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let assets = stmt
        .query_map(&param_refs[..], |row| {
            Ok(Asset {
                id: row.get(0)?,
                filename: row.get(1)?,
                path: row.get(2)?,
                zip_entry: row.get(3)?,
                asset_type: row.get(4)?,
                format: row.get(5)?,
                file_size: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                duration_ms: row.get(9)?,
                sample_rate: row.get(10)?,
                channels: row.get(11)?,
                created_at: row.get(12)?,
                modified_at: row.get(13)?,
                processing_status: row.get(14)?,
                processing_error: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(assets)
}

/// Get asset count
#[tauri::command]
pub async fn get_asset_count(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    Ok(count)
}
