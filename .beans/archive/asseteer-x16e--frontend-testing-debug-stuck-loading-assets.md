---
# asseteer-x16e
title: Frontend testing & debug stuck Loading assets
status: completed
type: task
priority: high
created_at: 2026-03-17T06:13:02Z
updated_at: 2026-03-18T08:10:15Z
---

Investigate and fix the "Loading assets..." hang in the library search view, and set up frontend testing infrastructure.

## Bug: "Loading assets..." stays stuck

### Symptoms
- User opens app, searches in images tab → "Loading assets..." spinner stays forever
- Frontend is responsive (not a JS hang)
- 1-2 cores CPU usage (something actively working in background)
- No errors in frontend or backend console

### What we verified (all passing, 42 Rust tests)
- **DB concurrency is fine**: Concurrent read/write tests with separate pools (simulating SQL plugin + sqlx) pass with no deadlocks, even without busy_timeout
- **FTS5 performance is fine**: Search over 5000 assets takes 8ms
- **WAL mode works correctly**: Readers are not blocked by writers
- **WorkQueue stale handles bug was found and FIXED**: Workers now respawn after natural completion (`handles.retain(|h| !h.is_finished())` in `work_queue.rs`)
- **Lazy thumbnail processing is correct**: NULL thumbnail_data after processing, existing thumbnails preserved on re-process

### Diagnostic timing results (added to `assets.svelte.ts` and `thumbnails.svelte.ts`)
```
[loadAssets] getDatabase: 0.5ms
[loadAssets] dbSearchAssets: 155ms
[loadAssets] search returned 5001 results
```
The search query itself completes fine and returns 5001 results (MAX_DISPLAY_LIMIT + 1). The hang occurs AFTER this point. The subsequent code path is:

```typescript
this.hasMoreResults = result.length > MAX_DISPLAY_LIMIT; // true
if (this.hasMoreResults) {
    this.totalMatchingCount = await countSearchResults(db, ...); // ← no timing log here yet
    if (currentVersion !== this.searchVersion) return; // ← possible silent cancellation
}
this.assets = result.slice(0, MAX_DISPLAY_LIMIT); // ← 5000 Asset objects assigned
const count = await getAssetCount(db); // ← another query
```

### Possible causes still to investigate
1. **`countSearchResults` query slow or hanging** — runs `SELECT COUNT(*) FROM assets INNER JOIN assets_fts ...` with no LIMIT. Need to add timing log before/after to confirm. (User doubts this is the issue with ~200k assets)
2. **Reactivity storm from assigning 5000 assets** — `this.assets = result.slice(0, MAX_DISPLAY_LIMIT)` assigns 5000 Asset objects to a `$state` array. This triggers Svelte 5 reactivity which may cascade to derived values, component re-renders (ImageGrid with virtual scroll), and thumbnail requests. Could overwhelm the event loop.
3. **Thumbnail `processBatch` blocking IPC** — After 5000 images are assigned, ImageGrid renders, each visible image calls `requestThumbnail()`, which after 50ms calls `invoke('ensure_thumbnails', ...)`. If this Tauri command takes a long time (generating thumbnails for many images), it blocks the IPC. Timing logs added to `processBatch` in `thumbnails.svelte.ts`.
4. **searchVersion cancellation swallowing completion** — If `loadAssets` is called again (e.g., by a processing-complete event or reactivity re-trigger) while the first search is still running, the first search's `finally` block won't clear `isLoading` (version mismatch). The new search would need to complete to clear it.
5. **Something in the component rendering** — The ImageGrid or virtual scroll component receiving 5000 items might cause issues.

### Next steps
- [ ] Add timing logs around `countSearchResults`, `getAssetCount`, and the `this.assets = ...` assignment
- [ ] Check if the hang correlates with the number of results (try a very specific search that returns <100 results)
- [ ] Set up frontend testing (Vitest) to test the `loadAssets` flow, reactivity, and query functions
- [ ] Consider testing the Toolbar → searchAssets → loadAssets pipeline
- [ ] Test `processBatch` thumbnail loading pipeline

## Frontend Testing Setup

### Recommended stack
- **Vitest** (already compatible with Vite 6 config)
- Mock `@tauri-apps/plugin-sql` and `@tauri-apps/api/core` (invoke)
- Test the state modules (`assets.svelte.ts`, `thumbnails.svelte.ts`) and query builders

### What to test
- `searchAssets` / `loadAssets` state transitions (isLoading flag lifecycle)
- `searchVersion` cancellation logic (concurrent searches)
- `processBatch` thumbnail loading (cache/pending/failed state)
- Query builder functions in `queries.ts` (SQL generation correctness)
- `buildRootNodes` / `buildChildNodes` pure functions
- Duration filter application

### Key files
- `src/lib/state/assets.svelte.ts` — has diagnostic `console.time` logs added
- `src/lib/state/thumbnails.svelte.ts` — has diagnostic `console.time` logs added
- `src/lib/database/queries.ts` — all SQL query functions
- `src/lib/database/connection.ts` — DB singleton
- `src-tauri/src/task_system/work_queue.rs` — stale handles fix applied + 11 tests
- `src-tauri/src/concurrent_tests.rs` — 4 concurrent access tests
- `src-tauri/src/test_helpers.rs` — shared test utilities (in-memory + file-backed DB, fixture generators)
