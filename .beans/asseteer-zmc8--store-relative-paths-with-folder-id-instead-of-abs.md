---
# asseteer-zmc8
title: Store relative paths with folder_id instead of absolute paths
status: todo
type: task
priority: normal
created_at: 2026-03-17T08:44:22Z
updated_at: 2026-03-17T08:55:58Z
parent: asseteer-i459
blocked_by:
    - asseteer-wxak
---

Full absolute paths (~60-100 bytes each) are stored in multiple places:
- `assets.path` (the raw path)
- `assets_fts.path_segments` (path with separators→spaces, via trigger)
- `idx_assets_path` (B-tree index)
- `idx_assets_unique` (another B-tree with path)

A typical Windows path is ~80 bytes. With 100K assets that's ~8MB per copy, ~30MB+ total just for paths.

## Proposed (ties into asseteer-wxak source_folders)

```sql
CREATE TABLE assets (
    ...
    folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    rel_path TEXT NOT NULL,  -- relative dir path within folder (~20-40 bytes)
    ...
);
CREATE UNIQUE INDEX idx_assets_unique ON assets(folder_id, rel_path, COALESCE(zip_entry, ''));
CREATE INDEX idx_assets_folder ON assets(folder_id);
```

Reconstruct full path when needed: `source_folders.path || sep || assets.rel_path`.

## Dependencies
- Requires `source_folders` table from asseteer-wxak

## Impact
- **Filesize:** ~50-60% reduction in path storage (shorter strings × fewer bytes in indexes)
- **Performance:** Smaller indexes = faster seeks, less I/O


## Future consideration: directory-filtered search

The `folder_id` + `rel_path` design naturally supports directory-scoped search:
- **Filter to source folder**: `WHERE folder_id = ?` (index-backed)
- **Filter to subdirectory**: `WHERE rel_path LIKE 'subdir/%'` (uses idx_assets_folder index)
- **Combined with FTS**: `WHERE folder_id = ? AND id IN (SELECT rowid FROM assets_fts WHERE ...)`

The FTS trigger should index only the directory components of `rel_path` (strip the filename) into a `dir_segments` column, since the filename is already indexed separately.
