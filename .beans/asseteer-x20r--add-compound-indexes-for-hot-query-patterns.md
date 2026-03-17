---
# asseteer-x20r
title: Add compound indexes for hot query patterns
status: todo
type: task
priority: normal
created_at: 2026-03-17T08:44:22Z
updated_at: 2026-03-17T08:55:55Z
parent: asseteer-i459
---

Current indexes are single-column and miss the most common query patterns.

## Hot queries lacking good indexes

1. **Search filtered by type:** `WHERE asset_type = ? AND id IN (FTS subquery)`
   - `idx_assets_type` has only 2 values (image/audio) — very low selectivity alone

2. **Pending processing:** `LEFT JOIN image_metadata ... WHERE asset_type = 'image' AND im.asset_id IS NULL`
   - Scans all assets of a type to find unprocessed ones

## Proposed indexes

```sql
-- Covers type-filtered searches (type + id for covering the FTS IN subquery)
CREATE INDEX idx_assets_type_id ON assets(asset_type, id);

-- If using folder_id: covers folder browsing
CREATE INDEX idx_assets_folder ON assets(folder_id);
```

The existing `idx_assets_type` can be dropped if `idx_assets_type_id` is added (it's a prefix).

## Impact
- **Performance:** Faster filtered searches, faster pending-processing counts
- **Filesize:** Roughly neutral (replacing one index with a slightly wider one)


## Future consideration: directory-filtered search

For combined folder + FTS queries (`WHERE folder_id = ? AND id IN (SELECT rowid FROM assets_fts WHERE ...)`), a covering index on `(folder_id, id)` lets the query engine intersect folder-filtered IDs with FTS rowids without hitting the main table:

```sql
CREATE INDEX idx_assets_folder_id ON assets(folder_id, id);
```

This replaces the simpler `idx_assets_folder ON assets(folder_id)` proposed above.
