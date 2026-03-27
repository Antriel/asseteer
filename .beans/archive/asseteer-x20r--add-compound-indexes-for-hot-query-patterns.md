---
# asseteer-x20r
title: Add compound indexes for hot query patterns
status: completed
type: task
priority: normal
created_at: 2026-03-17T08:44:22Z
updated_at: 2026-03-27T07:51:01Z
parent: asseteer-i459
blocked_by:
    - asseteer-1vw2
---

## Benchmark results (2026-03-27, 1.5M assets: 1.19M images, 327K audio)

| # | Query | Time | Verdict |
|---|-------|------|---------|
| 1 | **Browse images (no search, ORDER BY filename)** | **6.3s** | **Very slow** |
| 2 | Count images (no search) | 30ms | OK |
| 3 | Search images (FTS + type) | 27ms | Fast |
| 4 | Count images (FTS + type) | 3ms | Fast |
| 5 | Pending images (no thumbs) | 91ms | Acceptable |
| 6 | Pending images (with thumbs) | 92ms | Acceptable |
| 7 | Pending audio | 19ms | Fast |
| 8 | Dir children (root, folder_id filter) | 38ms | OK |
| 9 | Dir children (parent_id filter) | <1ms | Fast |
| 10 | **Folder-filtered browse (folder_id + type + ORDER BY)** | **807ms** | **Slow** |
| 11 | **Browse audio (ORDER BY filename)** | **120ms** | Borderline |
| 12 | **Fetch pending images for processing** | **437ms** | Slow (batch, one-time) |

### Analysis

The bottleneck is **ORDER BY filename COLLATE NOCASE** on large result sets, not lookup selectivity.

- Query 1 scans 1.19M image rows via `idx_assets_type` then sorts them all to get the first 50 — **6.3 seconds**.
- Query 10 scans a whole folder then sorts — **807ms**.
- Query 11 sorts 327K audio rows — **120ms**.
- FTS queries (3, 4) are fast because the FTS subquery narrows results before the sort.
- Pending-count queries (5–7) are acceptable — LEFT JOIN + IS NULL on ~1M rows is ~90ms.

### Proposed indexes (revised)

```sql
-- Eliminates the sort for type-filtered browsing (the main bottleneck)
-- Covers: browse images, browse audio, count by type
CREATE INDEX idx_assets_type_filename ON assets(asset_type, filename COLLATE NOCASE);

-- Eliminates the sort for folder-filtered browsing
-- Covers: folder browse with ORDER BY filename
CREATE INDEX idx_assets_folder_type_filename ON assets(folder_id, asset_type, filename COLLATE NOCASE);
```

The existing `idx_assets_type` and `idx_assets_folder` can be dropped — both are prefixes of the new indexes.

### Original proposed indexes (no longer recommended)

- `idx_assets_type_id` — would not help the sort bottleneck
- `idx_assets_folder_id` — same, sort is the issue not the lookup


## Summary of Changes

Replaced two single-column indexes with compound indexes that include `filename COLLATE NOCASE` to eliminate sort overhead on the main browsing queries:

- `idx_assets_type` → `idx_assets_type_filename ON assets(asset_type, filename COLLATE NOCASE)`
- `idx_assets_folder` → `idx_assets_folder_type_filename ON assets(folder_id, asset_type, filename COLLATE NOCASE)`

Old indexes are dropped via `MIGRATE_ASSETS_INDEXES` in `init.rs` (runs on startup, idempotent).

**Files changed:**
- `src-tauri/src/database/schema.rs` — new compound index definitions + migration constant
- `src-tauri/src/database/init.rs` — runs migration to drop old indexes
