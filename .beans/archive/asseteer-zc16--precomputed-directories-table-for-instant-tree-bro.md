---
# asseteer-zc16
title: Precomputed directories table for instant tree browsing
status: completed
type: feature
priority: high
created_at: 2026-03-26T10:23:06Z
updated_at: 2026-03-26T10:30:50Z
---

Replace expensive aggregation queries (getFolderChildren, getZipDirectoryChildren, get_zip_dir_trees) with a precomputed `directories` table populated at scan time. Currently expanding a folder takes 2-3s because it scans millions of asset rows. With this table, it becomes a simple `WHERE parent_id = ?` lookup.

## Tasks

- [x] Add `directories` table schema + indexes to schema.rs
- [x] Write `populate_directories()` Rust function
- [x] Wire into scan (add_folder) and rescan (apply_rescan)
- [x] Replace frontend queries (getFolderChildren, getZipDirectoryChildren, getSourceFolderRoots)
- [x] Simplify explore.svelte.ts to use directoryId-based lookups
- [x] Replace SearchConfigPanel's get_zip_dir_trees with DB query
- [x] Delete old code (Rust command + old query functions)


## Summary of Changes

### Backend (Rust)
- **`schema.rs`**: Added `directories` table with `parent_id` for tree structure, supporting `dir`, `zip`, and `zipdir` node types
- **`init.rs`**: Added table + index creation to DB setup
- **`scan.rs`**: Added `populate_directories()` function that queries asset rows, builds directory tree in memory, and bulk-inserts via rusqlite. Called after FTS population in `add_folder`
- **`rescan.rs`**: Calls `populate_directories()` after rescan transaction commits
- **`folders.rs`**: Removed `get_zip_dir_trees` command (replaced by frontend DB query)
- **`lib.rs`**: Removed `get_zip_dir_trees` from command registration

### Frontend (TypeScript/Svelte)
- **`queries.ts`**: Added `directoryId` to `DirectoryNode` interface. Replaced `getFolderChildren` (3 heavy queries) and `getZipDirectoryChildren` with single `getDirectoryChildren(db, directoryId, folderId)` using `WHERE parent_id = ?`. Added `getZipDirTrees()` frontend query replacing the Rust invoke. Updated `getDistinctRelPaths()` to read from directories table.
- **`explore.svelte.ts`**: Simplified `loadChildren` to single code path. Updated `toggleExpanded` and `navigateToAsset` to use `directoryId` lookups.
- **`DirectoryTree.svelte`**: Updated `toggleExpanded` call signature
- **`SearchConfigPanel.svelte`**: Replaced `invoke('get_zip_dir_trees')` with `getZipDirTrees(db, folderId)`
