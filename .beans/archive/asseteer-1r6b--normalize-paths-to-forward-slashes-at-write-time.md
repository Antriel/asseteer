---
# asseteer-1r6b
title: Normalize paths to forward slashes at write time
status: scrapped
type: task
priority: low
created_at: 2026-03-17T08:44:22Z
updated_at: 2026-03-19T06:44:47Z
parent: asseteer-i459
blocked_by:
    - asseteer-zmc8
---

Currently paths are stored with native OS separators (backslashes on Windows). Frontend queries must use `REPLACE(path, '\', '/')` to normalize, which **defeats index usage** and causes full table scans.

The Explore view fix (current commit) works around this by querying with native separators, but this is fragile — any code that normalizes a path before querying will break.

## Proposed
Normalize all paths to forward slashes at insert time in Rust (`scan.rs:466`):
```rust
let path_str = asset.path.to_string_lossy().replace('\', "/");
```

This ensures:
- All `WHERE path = ?` and `WHERE path LIKE ?` queries use `idx_assets_path` directly
- No REPLACE() needed in any query
- Frontend can always work with `/` separators consistently

## Note
If switching to relative paths (folder_id + rel_path), the same principle applies: normalize rel_path at write time. Also normalize `source_folders.path`.

## Impact
- **Performance:** Eliminates all REPLACE()-based full table scans
- **DX:** Simplifies query code on both frontend and backend


## Note: also needed for folder tree

The folder tree code currently uses `pathSep()` and `splitPath()` to handle both `/` and `\` separators. Normalizing to forward slashes at write time would simplify all tree-building logic significantly, since `splitPath` wouldn't need to detect the separator style.


## Reasons for Scrapping

Absorbed into asseteer-wxak. With the schema redesign (no migration), paths will be normalized to forward slashes from day one as part of the `rel_path` computation. No separate task needed.
