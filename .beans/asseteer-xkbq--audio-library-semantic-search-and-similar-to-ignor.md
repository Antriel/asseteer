---
# asseteer-xkbq
title: 'Audio library: semantic search and ''similar to'' ignore selected folder filter'
status: completed
type: bug
priority: normal
created_at: 2026-03-19T11:40:57Z
updated_at: 2026-03-20T09:06:37Z
parent: asseteer-kvnt
---

In the audio Library view, when semantic search is active AND a specific folder is selected in the sidebar, the folder filter is ignored — search runs across the entire library. Same issue with 'similar to' search. Non-semantic (text) search respects the folder filter correctly. The folder condition is likely not being passed through to the semantic/embedding query path.

## Summary of Changes

Post-filter semantic search results by `folderLocation` on the frontend.

- Added `filterByFolderLocation()` helper in `clap.svelte.ts` that mirrors the SQL logic in `addFolderFilterConditions()` — handles both `folder` and `zip` location types including prefix matching
- Added `folderLocation` param to `ClapState.search()` and `ClapState.searchBySimilarity()`; filter applied after results return from the backend
- Passed `assetsState.folderLocation` at all 4 call sites: `Toolbar.svelte`, `AudioList.svelte`, and 2 calls in `DurationFilter.svelte`
