---
# asseteer-j0dc
title: Separate thumbnail BLOBs from main database
status: todo
type: task
priority: high
created_at: 2026-03-17T08:44:21Z
updated_at: 2026-03-17T08:44:21Z
parent: asseteer-i459
---

Each WebP thumbnail is ~5-20KB. With 50K images that's 250MB-1GB of BLOBs interleaved with metadata rows in the main DB.

## Problems
- Bloats the DB file (slow backups/syncing, especially on Dropbox)
- Pollutes SQLite page cache with BLOB data during metadata queries
- Makes `VACUUM` very slow

## Options

### Option A: Separate SQLite database
```sql
-- Main DB
CREATE TABLE image_metadata (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    processed_at INTEGER NOT NULL
);

-- thumbnails.db (ATTACHed or opened separately)
CREATE TABLE thumbnails (
    asset_id INTEGER PRIMARY KEY,
    data BLOB NOT NULL
);
```

### Option B: Filesystem cache
Store as `{cache_dir}/thumbs/{asset_id}.webp`. Simplest, but more filesystem overhead.

### Recommendation
Option A — keeps atomic reads via SQL, allows the main DB to stay small. Thumbnails are regenerable cache data; losing them just means re-generating.

## Impact
- **Filesize:** Main DB shrinks dramatically (removes the largest data by far)
- **Performance:** Metadata queries no longer compete with BLOBs for page cache
