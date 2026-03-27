---
# asseteer-l8mm
title: 'Folders: ''Loading directory tree...'' is slow for large bundles'
status: completed
type: bug
priority: normal
created_at: 2026-03-23T15:43:13Z
updated_at: 2026-03-24T09:11:15Z
---

In the Folders page, the search indexing config step 'Loading directory tree...' takes a very long time for large imported bundles. This should query the DB (which is instant) rather than walking the actual filesystem. Needs investigation to confirm which path is used and fix if filesystem-based.


## Analysis

The loading is already DB-based (not filesystem walking). The bottleneck was an N+1 query pattern: `getDistinctZipFiles` returned all ZIP files, then `getDistinctZipDirs` was called **sequentially for each ZIP** to get its internal directory structure. For bundles with thousands of ZIPs, this meant thousands of sequential DB round-trips.

## Fix

Replaced the N+1 frontend query pattern (getDistinctZipFiles + per-zip getDistinctZipDirs) `getAllZipEntries()` that fetches all `(rel_path, zip_file, zip_entry)` tuples in one shot. Directory prefix extraction and grouping by zip now happens client-side in memory, which is essentially instant.

## Summary of Changes

- `queries.ts`: Removed `getDistinctZipFiles()` + `getDistinctZipDirs()` (and the intermediate `getAllZipEntries()`)
- `SearchConfigPanel.svelte`: Rewrote load() to call the Rust command instead of frontend DB queries for zip tree data
- `folders.rs`: Added `get_zip_dir_trees` Rust command that queries+groups in-process
- `schema.rs`: No schema changes (covering index was tested but rejected, see below)

## Investigation Notes

**Test case**: Humble Bundle folder — 1.49M assets, 1.38M inside 1,375 ZIP files.

### Bottleneck #1: N+1 frontend queries (~30s → ~9s)
Original code called `getDistinctZipFiles` then looped calling `getDistinctZipDirs` per ZIP = thousands of sequential Tauri SQL plugin round-trips. Fixed by batching into a single `getAllZipEntries()` frontend query. Still slow because transferring 1.38M rows through Tauri IPC (JSON serialization per row).

### Bottleneck #2: IPC serialization (~9s → ~8s)
Moved the entire operation to a Rust backend command (`get_zip_dir_trees`). Queries 1.38M rows directly in Rust, extracts directory prefixes, returns only ~1,212 compact groups. Eliminates IPC-per-row overhead. Profiling showed: SQL fetch 2.9s, grouping 5.0s.

### Bottleneck #3: Grouping logic (5.0s → 0.8s)
Original grouping used `HashMap<(String,String), BTreeSet<String>>` with cumulative prefix extraction per row = ~4M String allocations + BTreeSet insertions, mostly duplicates. Fixed by:
1. Collecting only leaf directories (not cumulative prefixes) — JS `buildTree()` creates intermediate nodes
2. Using `HashSet` with `contains(&str)` check before insert — avoids allocation for the 99%+ duplicate case
3. Processing rows sequentially (ORDER BY matches index order) — eliminates HashMap

### Rejected: Covering index (+281MB, saved ~2s)
Tested a partial covering index `idx_assets_zip_tree ON assets(folder_id, rel_path, zip_file, zip_entry) WHERE zip_file IS NOT NULL AND zip_entry IS NOT NULL`. Reduced SQL fetch from ~3s to ~1s but added 281MB (~10%) to the 2.8GB database. Dropped because the feature is infrequently used (only when expanding "Search indexing" panel) and the improvement wasn't worth the storage cost.

### Final result: ~30s → ~4.5s (no extra storage)
