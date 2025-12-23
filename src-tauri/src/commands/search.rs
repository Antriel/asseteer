//! CLAP-based semantic search and embedding commands

use crate::clap::{
    blob_to_embedding, cosine_similarity, embedding_to_blob, ensure_server_running, get_clap_client,
};
use crate::models::Asset;
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
        JOIN audio_metadata am ON a.id = am.asset_id
        LEFT JOIN audio_embeddings ae ON a.id = ae.asset_id
        WHERE a.asset_type = 'audio' AND ae.asset_id IS NULL
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.0)
}

/// Process CLAP embeddings for audio assets that have metadata but no embedding
#[tauri::command]
pub async fn process_clap_embeddings(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ProcessClapResult, String> {
    use crate::utils::load_asset_bytes;

    // Ensure CLAP server is running
    ensure_server_running().await?;

    // Get audio assets that have metadata but no CLAP embedding
    let limit_clause = match limit {
        Some(n) => format!("LIMIT {}", n),
        None => String::new(),
    };

    let query = format!(
        r#"
        SELECT a.* FROM assets a
        JOIN audio_metadata am ON a.id = am.asset_id
        LEFT JOIN audio_embeddings ae ON a.id = ae.asset_id
        WHERE a.asset_type = 'audio' AND ae.asset_id IS NULL
        ORDER BY a.id
        {}
        "#,
        limit_clause
    );

    let assets: Vec<Asset> = sqlx::query_as(&query)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| format!("Failed to query pending assets: {}", e))?;

    if assets.is_empty() {
        return Ok(ProcessClapResult {
            processed: 0,
            failed: 0,
            errors: vec![],
        });
    }

    let total = assets.len();
    let mut processed = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    println!(
        "[CLAP] Processing embeddings for {} audio assets",
        total
    );

    let client = get_clap_client().await;

    for asset in assets {
        // Generate embedding
        let embedding_result = if asset.zip_entry.is_some() {
            // Audio inside ZIP - send raw bytes
            match load_asset_bytes(&asset) {
                Ok(bytes) => client.embed_audio_bytes(bytes, &asset.filename).await,
                Err(e) => Err(format!("Failed to load asset bytes: {}", e)),
            }
        } else {
            // Regular file - send path
            client.embed_audio_path(&asset.path).await
        };

        match embedding_result {
            Ok(embedding) => {
                // Store embedding
                let blob = embedding_to_blob(&embedding);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;

                let result = sqlx::query(
                    "INSERT INTO audio_embeddings (asset_id, embedding, created_at)
                     VALUES (?, ?, ?)
                     ON CONFLICT (asset_id) DO UPDATE SET
                         embedding = excluded.embedding,
                         created_at = excluded.created_at",
                )
                .bind(asset.id)
                .bind(&blob)
                .bind(now)
                .execute(&state.pool)
                .await;

                if let Err(e) = result {
                    failed += 1;
                    errors.push(format!("{}: DB error: {}", asset.filename, e));
                } else {
                    processed += 1;
                }
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", asset.filename, e));
            }
        }
    }

    println!(
        "[CLAP] Completed: {} processed, {} failed",
        processed, failed
    );

    Ok(ProcessClapResult {
        processed,
        failed,
        errors,
    })
}

/// Result of CLAP embedding processing
#[derive(Debug, Clone, Serialize)]
pub struct ProcessClapResult {
    pub processed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
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
