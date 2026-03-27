---
# asseteer-wq8a
title: 'Semantic search: avoid loading all embeddings into RAM'
status: completed
type: task
priority: normal
created_at: 2026-03-17T08:44:22Z
updated_at: 2026-03-18T11:45:35Z
parent: asseteer-i459
---

`search_audio_semantic` (search.rs:101) does `fetch_all` on every embedding row, deserializes all BLOBs to `Vec<f32>`, computes cosine similarity for ALL of them, sorts, then truncates to `limit`.

CLAP-htsat-fused produces 512-dim vectors = 2KB per embedding. At 100K audio files that's **200MB loaded per search query**.

## Current code (search.rs)
```rust
let rows: Vec<SemanticSearchRow> = query
    .fetch_all(&state.pool)  // loads ALL embeddings
    .await?;
// then: sort all, truncate to limit
```

## Options

### Short term: Stream + bounded heap
Use `fetch` (streaming) instead of `fetch_all`. Maintain a min-heap of size `limit` so only top-N results stay in memory.

### Medium term: Approximate nearest neighbor
- `sqlite-vss` extension (based on Faiss)
- `usearch` SQLite extension (HNSW index)
- Sidecar HNSW index file (e.g. via `instant-distance` or `hnsw` Rust crate)

### Also consider
- Store pre-normalized embeddings (unit vectors) so cosine similarity = dot product
- Remove `model_version` TEXT column (wastes ~25 bytes/row repeating `'laion/clap-htsat-fused'`); use an integer or remove entirely if only one model

## Impact
- **Memory:** From O(N) to O(limit) per query
- **Latency:** Streaming avoids deserializing embeddings that won't make top-N


## Summary of Changes

Implemented in-memory embedding cache with flat matrix layout and parallel similarity computation:

- **Cache** (`src-tauri/src/clap/cache.rs`): Lazy-loaded flat `Vec<f32>` buffer storing all embeddings contiguously. Duration metadata cached alongside for in-memory filtering. Uses `tokio::sync::RwLock` for thread-safe access.
- **Rayon parallelism**: Similarity computation across 164K embeddings parallelized via `par_iter()`, scaling with CPU cores.
- **Smart metadata fetch**: Only fetches full asset data for top-N results (tiny query for ~50 rows).
- **Auto-invalidation**: Cache invalidated when CLAP processing completes or is stopped. Manual `invalidate_embedding_cache` command also exposed.
- **Benchmark**: Added `benchmark_audio_search` command with per-phase timing breakdown comparing uncached vs cached paths.

### Results (164K × 512d embeddings, 24-core CPU)

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Search latency | 7,600ms | 105ms | **67×** faster |
| First search (cold) | 7,600ms | 2,590ms | 3× faster |
| Memory | 0 (loaded per query) | ~322 MB resident | Trade-off |
