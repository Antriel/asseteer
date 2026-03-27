---
# asseteer-u7mb
title: Missing SQLite variable limit chunking in thumbnail_worker.rs
status: completed
type: bug
priority: normal
created_at: 2026-03-20T11:45:21Z
updated_at: 2026-03-21T08:28:03Z
parent: asseteer-c0lx
---

`thumbnail_worker.rs` builds IN-clause queries in `find_missing_thumbnails` (line 371) and `load_assets` (line 398) without chunking. SQLite has a default variable limit of 999.

Meanwhile, `search.rs:fetch_asset_metadata` (line 67) correctly chunks at 999:
```rust
for chunk in ids.chunks(999) { ... }
```

If the user scrolls through a large library quickly, the thumbnail worker could receive >999 asset IDs in a single batch, causing an SQLite error.

**Fix**: Add `ids.chunks(999)` batching to both `find_missing_thumbnails` and `load_assets`, matching the pattern already used in `search.rs`.

## Summary of Changes

Added `ids.chunks(999)` batching to both `find_missing_thumbnails` and `load_assets` in `thumbnail_worker.rs`, matching the existing pattern in `search.rs`. Both functions now iterate over chunks and accumulate results, preventing SQLite's 999-variable limit from being exceeded when processing large batches.
