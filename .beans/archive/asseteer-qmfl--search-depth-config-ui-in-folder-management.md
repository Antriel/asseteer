---
# asseteer-qmfl
title: Search depth config UI in folder management
status: completed
type: feature
priority: normal
created_at: 2026-03-19T10:44:00Z
updated_at: 2026-03-20T10:38:41Z
parent: asseteer-i459
---

Add UI to folder management page for configuring per-folder search depth (skip_depth). Backend update_search_config command already exists. Users need to see subfolder tree and configure which path segments to skip from FTS indexing.


## Implementation Plan

- [x] Add `SearchConfigEntry` type to `src/lib/types/index.ts`
- [x] Add `getSearchConfig()`, `getTopLevelSubfolders()`, and `getSampleAssetPath()` queries to `queries.ts`
- [x] Build expandable `SearchConfigPanel` component in folders page with:
  - Root-level skip_depth control
  - List of subfolder-specific rules with add/edit/remove
  - Example preview showing original → indexed path
  - Save button that calls `update_search_config` backend command
  - Re-index progress indicator
