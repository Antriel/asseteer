---
# asseteer-wq8a
title: 'Semantic search: avoid loading all embeddings into RAM'
status: todo
type: task
priority: normal
created_at: 2026-03-17T08:44:22Z
updated_at: 2026-03-17T08:44:22Z
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
