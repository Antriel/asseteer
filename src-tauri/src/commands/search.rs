//! CLAP-based semantic search commands

use crate::clap::{blob_to_embedding, cosine_similarity, ensure_server_running, get_clap_client, HealthInfo};
use crate::AppState;
use serde::Serialize;
use tauri::State;

/// Result of a semantic search query - includes full asset data for direct use
#[derive(Debug, Clone, Serialize)]
pub struct SemanticSearchResult {
    // Asset fields
    pub id: i64,
    pub filename: String,
    pub path: String,
    pub zip_entry: Option<String>,
    pub asset_type: String,
    pub format: String,
    pub file_size: i64,
    pub created_at: i64,
    pub modified_at: i64,
    // Audio metadata (nullable)
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    // Similarity score
    pub similarity: f32,
}

/// Row returned from semantic search query
#[derive(sqlx::FromRow)]
struct SemanticSearchRow {
    id: i64,
    filename: String,
    path: String,
    zip_entry: Option<String>,
    asset_type: String,
    format: String,
    file_size: i64,
    created_at: i64,
    modified_at: i64,
    duration_ms: Option<i64>,
    sample_rate: Option<i32>,
    channels: Option<i32>,
    embedding: Vec<u8>,
}

/// Semantic search for audio assets using CLAP embeddings
#[tauri::command]
pub async fn search_audio_semantic(
    query: String,
    limit: usize,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<SemanticSearchResult>, String> {
    // Ensure server is running
    ensure_server_running().await?;

    // Get query embedding
    let query_embedding = get_clap_client().await.embed_text(&query).await?;

    // Build WHERE clause for duration filtering
    let mut where_clauses: Vec<String> = vec![];
    if min_duration_ms.is_some() {
        where_clauses.push("am.duration_ms >= ?".to_string());
    }
    if max_duration_ms.is_some() {
        where_clauses.push("am.duration_ms <= ?".to_string());
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let sql = format!(
        r#"
        SELECT
            a.id, a.filename, a.path, a.zip_entry, a.asset_type, a.format,
            a.file_size, a.created_at, a.modified_at,
            am.duration_ms, am.sample_rate, am.channels,
            ae.embedding
        FROM assets a
        JOIN audio_embeddings ae ON a.id = ae.asset_id
        LEFT JOIN audio_metadata am ON a.id = am.asset_id
        {}
        "#,
        where_clause
    );

    // Build query with optional bindings
    let mut query = sqlx::query_as::<_, SemanticSearchRow>(&sql);
    if let Some(min) = min_duration_ms {
        query = query.bind(min);
    }
    if let Some(max) = max_duration_ms {
        query = query.bind(max);
    }

    let rows: Vec<SemanticSearchRow> = query
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    // Compute similarities
    let mut results: Vec<SemanticSearchResult> = rows
        .into_iter()
        .map(|row| {
            let embedding = blob_to_embedding(&row.embedding);
            let similarity = cosine_similarity(&query_embedding, &embedding);
            SemanticSearchResult {
                id: row.id,
                filename: row.filename,
                path: row.path,
                zip_entry: row.zip_entry,
                asset_type: row.asset_type,
                format: row.format,
                file_size: row.file_size,
                created_at: row.created_at,
                modified_at: row.modified_at,
                duration_ms: row.duration_ms,
                sample_rate: row.sample_rate,
                channels: row.channels,
                similarity,
            }
        })
        .collect();

    // Sort by similarity descending
    results.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
    results.truncate(limit);

    Ok(results)
}

/// Get count of audio assets pending CLAP embedding
#[tauri::command]
pub async fn get_pending_clap_count(state: State<'_, AppState>) -> Result<i64, String> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM assets a
        LEFT JOIN audio_embeddings ae ON a.id = ae.asset_id
        WHERE a.asset_type = 'audio' AND ae.asset_id IS NULL
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.0)
}

/// Check if CLAP server is available
#[tauri::command]
pub async fn check_clap_server() -> Result<bool, String> {
    match get_clap_client().await.health_check().await {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Start the CLAP server if not running
#[tauri::command]
pub async fn start_clap_server() -> Result<(), String> {
    ensure_server_running().await
}

/// Get detailed CLAP server health info (device, model, etc.)
#[tauri::command]
pub async fn get_clap_server_info() -> Result<HealthInfo, String> {
    get_clap_client().await.health_check_detailed().await
}

/// Get the size of the uv cache directory in bytes
#[tauri::command]
pub fn get_clap_cache_size() -> Result<u64, String> {
    let cache_dir = crate::clap::uv::uv_cache_dir();
    if !cache_dir.exists() {
        return Ok(0);
    }
    dir_size(&cache_dir).map_err(|e| format!("Failed to calculate cache size: {}", e))
}

/// Clear the uv cache (downloaded Python, packages, etc.)
#[tauri::command]
pub async fn clear_clap_cache() -> Result<(), String> {
    // Stop the server first so it releases file locks on the cache directory
    crate::clap::stop_server_and_wait().await;

    let cache_dir = crate::clap::uv::uv_cache_dir();
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to clear cache: {}", e))?;
    }
    // Also remove the uv binary so it re-downloads fresh
    let uv_path = crate::clap::uv::uv_bin_path();
    if uv_path.exists() {
        std::fs::remove_file(&uv_path)
            .map_err(|e| format!("Failed to remove uv binary: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_clap_log_dir() -> String {
    crate::clap::log_dir().to_string_lossy().into_owned()
}

/// Check what CLAP setup artifacts exist on disk
#[derive(Serialize)]
pub struct ClapSetupState {
    pub uv_installed: bool,
    pub cache_exists: bool,
}

#[tauri::command]
pub fn check_clap_setup_state() -> ClapSetupState {
    let uv_installed = crate::clap::uv::uv_bin_path().exists();
    let cache_dir = crate::clap::uv::uv_cache_dir();
    let cache_exists = cache_dir.exists()
        && cache_dir
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);
    ClapSetupState {
        uv_installed,
        cache_exists,
    }
}

fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                total += dir_size(&entry.path())?;
            } else {
                total += meta.len();
            }
        }
    }
    Ok(total)
}
