//! CLAP-based semantic search commands

use crate::clap::{cache as embedding_cache, ensure_server_running, get_clap_client, HealthInfo};
use crate::clap::blob_to_embedding;
use crate::clap::cache::FolderFilter;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

/// Result of a semantic search query - includes full asset data for direct use
#[derive(Debug, Clone, Serialize)]
pub struct SemanticSearchResult {
    // Asset fields
    pub id: i64,
    pub filename: String,
    pub folder_id: i64,
    pub rel_path: String,
    pub zip_file: Option<String>,
    pub zip_entry: Option<String>,
    pub folder_path: String,
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

/// Row for fetching asset metadata (no embedding).
#[derive(sqlx::FromRow)]
struct AssetMetadataRow {
    id: i64,
    filename: String,
    folder_id: i64,
    rel_path: String,
    zip_file: Option<String>,
    zip_entry: Option<String>,
    folder_path: String,
    asset_type: String,
    format: String,
    file_size: i64,
    created_at: i64,
    modified_at: i64,
    duration_ms: Option<i64>,
    sample_rate: Option<i32>,
    channels: Option<i32>,
}

/// Build `SemanticSearchResult` list from ranked results and a metadata map.
fn build_search_results(
    ranked: Vec<crate::clap::cache::SimilarityResult>,
    metadata: &HashMap<i64, AssetMetadataRow>,
) -> Vec<SemanticSearchResult> {
    ranked
        .into_iter()
        .filter_map(|r| {
            metadata.get(&r.asset_id).map(|m| SemanticSearchResult {
                id: m.id,
                filename: m.filename.clone(),
                folder_id: m.folder_id,
                rel_path: m.rel_path.clone(),
                zip_file: m.zip_file.clone(),
                zip_entry: m.zip_entry.clone(),
                folder_path: m.folder_path.clone(),
                asset_type: m.asset_type.clone(),
                format: m.format.clone(),
                file_size: m.file_size,
                created_at: m.created_at,
                modified_at: m.modified_at,
                duration_ms: m.duration_ms,
                sample_rate: m.sample_rate,
                channels: m.channels,
                similarity: r.similarity,
            })
        })
        .collect()
}

/// Fetch full asset metadata for a set of asset IDs.
/// Returns a map from asset_id to row data.
async fn fetch_asset_metadata(
    ids: &[i64],
    pool: &sqlx::SqlitePool,
) -> Result<HashMap<i64, AssetMetadataRow>, String> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    // SQLite has a variable limit, batch in chunks of 999
    let mut map = HashMap::with_capacity(ids.len());
    for chunk in ids.chunks(999) {
        let placeholders: String = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            r#"
            SELECT a.id, a.filename, a.folder_id, a.rel_path, a.zip_file, a.zip_entry,
                   sf.path as folder_path,
                   a.asset_type, a.format, a.file_size, a.created_at, a.modified_at,
                   am.duration_ms, am.sample_rate, am.channels
            FROM assets a
            JOIN source_folders sf ON a.folder_id = sf.id
            LEFT JOIN audio_metadata am ON a.id = am.asset_id
            WHERE a.id IN ({})
            "#,
            placeholders
        );
        let mut query = sqlx::query_as::<_, AssetMetadataRow>(&sql);
        for &id in chunk {
            query = query.bind(id);
        }
        let rows: Vec<AssetMetadataRow> = query
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
        for row in rows {
            map.insert(row.id, row);
        }
    }
    Ok(map)
}

/// Semantic search for audio assets using CLAP embeddings
#[tauri::command]
pub async fn search_audio_semantic(
    query: String,
    limit: usize,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    folder_filter: Option<FolderFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SemanticSearchResult>, String> {
    // Ensure server is running
    ensure_server_running().await?;

    // Get query embedding from CLAP server
    let query_embedding = get_clap_client().await.embed_text(&query).await?;

    // Use cached embeddings for similarity search
    let ranked = embedding_cache::search(
        &query_embedding,
        limit,
        None,
        min_duration_ms,
        max_duration_ms,
        folder_filter.as_ref(),
        &state.pool,
    )
    .await?;

    // Fetch full metadata only for top results
    let ids: Vec<i64> = ranked.iter().map(|r| r.asset_id).collect();
    let metadata = fetch_asset_metadata(&ids, &state.pool).await?;

    // Build results preserving similarity order, skipping any deleted assets
    Ok(build_search_results(ranked, &metadata))
}

/// Find audio assets similar to a given audio asset using its stored CLAP embedding
#[tauri::command]
pub async fn search_audio_by_similarity(
    asset_id: i64,
    limit: usize,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    folder_filter: Option<FolderFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SemanticSearchResult>, String> {
    // Fetch the source asset's embedding
    let source_row: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT embedding FROM audio_embeddings WHERE asset_id = ?",
    )
    .bind(asset_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let source_embedding = match source_row {
        Some((blob,)) => blob_to_embedding(&blob),
        None => return Err("This asset hasn't been processed yet".to_string()),
    };

    // Use cached embeddings for similarity search
    let ranked = embedding_cache::search(
        &source_embedding,
        limit,
        None,
        min_duration_ms,
        max_duration_ms,
        folder_filter.as_ref(),
        &state.pool,
    )
    .await?;

    // Fetch full metadata only for top results
    let ids: Vec<i64> = ranked.iter().map(|r| r.asset_id).collect();
    let metadata = fetch_asset_metadata(&ids, &state.pool).await?;

    // Build results preserving similarity order, skipping any deleted assets
    Ok(build_search_results(ranked, &metadata))
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

/// Check if the CLAP server is available.
///
/// Only health-checks the server if we actually launched it this session —
/// we never probe arbitrary ports to avoid hitting unrelated services.
#[tauri::command]
pub async fn check_clap_server() -> Result<bool, String> {
    use crate::clap::is_server_running;
    if !is_server_running().await {
        return Ok(false);
    }
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

/// Invalidate the in-memory embedding cache, forcing a reload on next search.
#[tauri::command]
pub fn invalidate_embedding_cache() {
    embedding_cache::invalidate();
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
