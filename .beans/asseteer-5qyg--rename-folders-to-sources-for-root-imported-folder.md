---
# asseteer-5qyg
title: Rename 'Folders' to 'Sources' for root imported folders
status: completed
type: task
priority: normal
created_at: 2026-03-26T11:12:21Z
updated_at: 2026-03-26T11:13:45Z
---

Rename the root imported folders concept from 'Folders' to 'Sources' throughout the UI. Keep 'Folders' where it refers to actual filesystem folders within sources (FolderSidebar, TabBar toggle).


## Summary of Changes

- Renamed nav item label and route from `/folders` → `/sources` in `Sidebar.svelte`
- Updated icon type `'folders'` → `'sources'` (type definition + rendering condition) in `Sidebar.svelte`
- Updated "Folders" section heading → "Sources" above the roots list in `Sidebar.svelte`
- Updated `StatusBar.svelte` scan link href
- Updated `+page.svelte` (root) redirect logic
- Updated `scan/+page.svelte` redirect
- Updated `processing/+page.svelte` CTA link
- Renamed page title in `sources/+page.svelte`
- Renamed route directory `(app)/folders/` → `(app)/sources/`
- Left "Folders" unchanged in `FolderSidebar.svelte` and `TabBar.svelte` (refers to filesystem folders within sources)
