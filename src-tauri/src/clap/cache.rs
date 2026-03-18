//! In-memory embedding cache for fast semantic search.
//!
//! Stores all embeddings in a flat contiguous f32 buffer for cache-friendly
//! access and uses rayon to parallelize similarity computation across cores.

use super::embedding::{blob_to_embedding, cosine_similarity};
use rayon::prelude::*;
use sqlx::SqlitePool;
use std::time::Instant;
use tokio::sync::RwLock;

/// Per-entry metadata stored alongside the flat embedding matrix.
struct EntryMeta {
    asset_id: i64,
    duration_ms: Option<i64>,
}

/// The loaded cache: flat embedding matrix + per-entry metadata.
struct LoadedCache {
    /// All embeddings concatenated: [e0_d0, e0_d1, ..., e0_dN, e1_d0, ...]
    embeddings: Vec<f32>,
    /// Dimensionality of each embedding (stride in the flat buffer).
    dim: usize,
    /// Per-entry metadata, parallel to embeddings (entry i starts at i * dim).
    meta: Vec<EntryMeta>,
}

/// Thread-safe embedding cache. None = not loaded yet or invalidated.
static CACHE: RwLock<Option<LoadedCache>> = RwLock::const_new(None);

/// Row type for the cache-loading query.
#[derive(sqlx::FromRow)]
struct CacheRow {
    asset_id: i64,
    embedding: Vec<u8>,
    duration_ms: Option<i64>,
}

/// Load all embeddings from the database into a flat matrix cache.
async fn load_cache(pool: &SqlitePool) -> Result<LoadedCache, String> {
    let start = Instant::now();

    let rows: Vec<CacheRow> = sqlx::query_as(
        r#"
        SELECT ae.asset_id, ae.embedding, am.duration_ms
        FROM audio_embeddings ae
        LEFT JOIN audio_metadata am ON ae.asset_id = am.asset_id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let count = rows.len();
    if count == 0 {
        return Ok(LoadedCache {
            embeddings: Vec::new(),
            dim: 0,
            meta: Vec::new(),
        });
    }

    // Determine dimensionality from first row
    let dim = rows[0].embedding.len() / 4;

    // Pre-allocate flat buffer
    let mut embeddings = vec![0.0f32; count * dim];
    let mut meta = Vec::with_capacity(count);

    for (i, row) in rows.iter().enumerate() {
        let emb = blob_to_embedding(&row.embedding);
        embeddings[i * dim..(i + 1) * dim].copy_from_slice(&emb);
        meta.push(EntryMeta {
            asset_id: row.asset_id,
            duration_ms: row.duration_ms,
        });
    }

    let elapsed = start.elapsed();
    println!(
        "[EmbeddingCache] Loaded {} × {}d embeddings ({:.0} MB) in {:.1}ms",
        count,
        dim,
        (count * dim * 4) as f64 / 1_048_576.0,
        elapsed.as_secs_f64() * 1000.0
    );

    Ok(LoadedCache {
        embeddings,
        dim,
        meta,
    })
}

/// Ensure the cache is populated, loading from DB if needed.
async fn ensure_loaded(pool: &SqlitePool) -> Result<(), String> {
    // Fast path: cache already loaded
    {
        let guard = CACHE.read().await;
        if guard.is_some() {
            return Ok(());
        }
    }

    // Slow path: need to load
    let loaded = load_cache(pool).await?;
    let mut guard = CACHE.write().await;
    // Double-check after acquiring write lock
    if guard.is_none() {
        *guard = Some(loaded);
    }
    Ok(())
}

/// Invalidate the cache, forcing a reload on next search.
pub fn invalidate() {
    // Use try_write to avoid blocking. If we can't get the lock,
    // someone is actively using the cache — that's fine, we'll
    // invalidate on next opportunity.
    if let Ok(mut guard) = CACHE.try_write() {
        if guard.is_some() {
            println!("[EmbeddingCache] Invalidated");
            *guard = None;
        }
    } else {
        // Spawn a task to invalidate when the lock is free
        tokio::spawn(async {
            let mut guard = CACHE.write().await;
            if guard.is_some() {
                println!("[EmbeddingCache] Invalidated (deferred)");
                *guard = None;
            }
        });
    }
}

/// Result of a cached similarity search: asset IDs ranked by similarity.
pub struct SimilarityResult {
    pub asset_id: i64,
    pub similarity: f32,
}

/// Search the cache using a query embedding with optional duration filter.
/// Returns top `limit` results sorted by similarity descending.
///
/// Uses rayon to parallelize dot product computation across CPU cores.
pub async fn search(
    query_embedding: &[f32],
    limit: usize,
    exclude_asset_id: Option<i64>,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    pool: &SqlitePool,
) -> Result<Vec<SimilarityResult>, String> {
    ensure_loaded(pool).await?;

    let guard = CACHE.read().await;
    let cache = guard.as_ref().unwrap();
    let dim = cache.dim;

    if dim == 0 {
        return Ok(Vec::new());
    }

    // Clone what we need for the rayon closure (query embedding is small)
    let query = query_embedding.to_vec();
    let embeddings = &cache.embeddings;
    let meta = &cache.meta;

    // Parallel filter + similarity computation
    let mut results: Vec<SimilarityResult> = meta
        .par_iter()
        .enumerate()
        .filter(|(_, m)| {
            if let Some(exclude_id) = exclude_asset_id {
                if m.asset_id == exclude_id {
                    return false;
                }
            }
            if let Some(min) = min_duration_ms {
                match m.duration_ms {
                    Some(d) if d >= min => {}
                    _ => return false,
                }
            }
            if let Some(max) = max_duration_ms {
                match m.duration_ms {
                    Some(d) if d <= max => {}
                    _ => return false,
                }
            }
            true
        })
        .map(|(i, m)| {
            let emb = &embeddings[i * dim..(i + 1) * dim];
            SimilarityResult {
                asset_id: m.asset_id,
                similarity: cosine_similarity(&query, emb),
            }
        })
        .collect();

    // Sort and truncate (single-threaded, fast on pre-computed scores)
    results.sort_unstable_by(|a, b| b.similarity.total_cmp(&a.similarity));
    results.truncate(limit);

    Ok(results)
}
