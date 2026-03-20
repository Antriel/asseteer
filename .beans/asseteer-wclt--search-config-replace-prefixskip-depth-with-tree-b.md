---
# asseteer-wclt
title: 'Search config: replace prefix+skip_depth with tree-based segment excludes'
status: completed
type: feature
priority: normal
created_at: 2026-03-20T08:44:41Z
updated_at: 2026-03-20T08:53:37Z
parent: asseteer-kvnt
---

Replace the current folder_search_config (prefix + skip_depth) model with a simpler folder_search_excludes model. Each excluded directory segment is stored as a row. The UI becomes a visual tree where users check/uncheck segments. Works for both filesystem paths and ZIP-internal paths.

## Schema change

Replace `folder_search_config` table with:

```sql
CREATE TABLE folder_search_excludes (
    id INTEGER PRIMARY KEY,
    source_folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    zip_file TEXT,
    excluded_path TEXT NOT NULL,
    UNIQUE(source_folder_id, COALESCE(zip_file, ''), excluded_path)
);
```

## Tasks

- [x] Replace DB schema: folder_search_config → folder_search_excludes
- [x] Rewrite compute_searchable_path to use excluded set instead of prefix+skip
- [x] Rewrite load_search_config → load_search_excludes
- [x] Update update_search_config command to accept excludes
- [x] Update frontend types (SearchConfigEntry → SearchExclude)
- [x] Update frontend queries (getSearchConfig, drop getTopLevelSubfolders/getSampleAssetPath)
- [x] Rewrite SearchConfigPanel.svelte as tree-based UI
- [x] Update queries.ts to add tree-loading query for search config

## Summary of Changes

Replaced folder_search_config (prefix + skip_depth) with folder_search_excludes (per-segment exclusion). Backend: new table schema, simplified compute_searchable_path using HashSet lookup instead of prefix matching, load_search_excludes returns a HashSet. Frontend: tree-based UI where users check/uncheck directory segments (filesystem and ZIP-internal), replaces the old abstract prefix+skip rules. All callers updated across scan, rescan, and folder commands.
