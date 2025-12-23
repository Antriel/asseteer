//! CLAP-based semantic search commands

use crate::clap::{blob_to_embedding, cosine_similarity, ensure_server_running, get_clap_client};
use crate::AppState;
use serde::Serialize;
use tauri::State;

/// Result of a semantic search query
#[derive(Debug, Clone, Serialize)]
pub struct SemanticSearchResult {
    pub asset_id: i64,
    pub filename: String,
    pub path: String,
    pub similarity: f32,
}

/// Semantic search for audio assets using CLAP embeddings
#[tauri::command]
pub async fn search_audio_semantic(
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<SemanticSearchResult>, String> {
    // Ensure server is running
    ensure_server_running().await?;

    // Get query embedding
    let query_embedding = get_clap_client().await.embed_text(&query).await?;

    // Fetch all embeddings from database
    let rows: Vec<(i64, String, String, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT a.id, a.filename, a.path, ae.embedding
        FROM assets a
        JOIN audio_embeddings ae ON a.id = ae.asset_id
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    // Compute similarities
    let mut results: Vec<SemanticSearchResult> = rows
        .iter()
        .map(|(id, filename, path, embedding_blob)| {
            let embedding = blob_to_embedding(embedding_blob);
            let similarity = cosine_similarity(&query_embedding, &embedding);
            SemanticSearchResult {
                asset_id: *id,
                filename: filename.clone(),
                path: path.clone(),
                similarity,
            }
        })
        .collect();

    // Sort by similarity descending
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
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
