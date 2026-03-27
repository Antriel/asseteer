---
# asseteer-vto5
title: Parallelize ensure_thumbnails generation
status: completed
type: task
priority: normal
created_at: 2026-03-16T11:42:44Z
updated_at: 2026-03-16T15:52:30Z
---

## Problem

`ensure_thumbnails` receives batches of 10-50+ asset IDs (from the 50ms frontend debounce window) but processes them **sequentially** in a for-loop. Each iteration does heavy CPU work: ZIP extraction → image decode → resize → WebP encode → DB write. On first view of a ZIP-packed folder, this creates a visible waterfall where thumbnails appear one by one.

## Proposed Fix

Parallelize the generation loop in `ensure_thumbnails` (src-tauri/src/commands/assets.rs). Each `generate_thumbnail_for_asset` call already uses `tokio::spawn_blocking`, so the simplest approach is to spawn all tasks concurrently and `join_all`:

```rust
let tasks: Vec<_> = assets.iter().map(|asset| {
    generate_thumbnail_for_asset(asset)
}).collect();
let results = futures::future::join_all(tasks).await;
```

Then write results to DB (can remain sequential — DB writes are fast).

## Considerations

- [x] Add concurrency limit (e.g. semaphore or `buffer_unordered(N)`) to avoid spawning 200 blocking tasks at once
- [x] Verify no contention on ZIP file handles when multiple tasks extract from the same ZIP simultaneously — each task opens its own file handle, no shared state
- [ ] Consider whether the frontend should also parallelize the post-generation DB reads (currently sequential `for` loop in `processBatch`) — deferred, DB reads are fast
- [ ] Benchmark before/after with a large ZIP folder (100+ images) — needs manual testing

## Summary of Changes

Replaced sequential for-loop in `ensure_thumbnails` (src-tauri/src/commands/assets.rs) with `tokio::task::JoinSet` + semaphore. Thumbnail generation now runs up to N tasks in parallel (where N = CPU count, minimum 2). DB writes remain sequential after all generation completes. No new dependencies — uses tokio JoinSet and Semaphore already available with `tokio = { features = ["full"] }`.
