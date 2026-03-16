//! CLAP-based semantic search commands

use crate::clap::{blob_to_embedding, cosine_similarity, ensure_server_running, get_clap_client};
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
