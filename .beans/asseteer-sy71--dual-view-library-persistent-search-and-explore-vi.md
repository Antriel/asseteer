---
# asseteer-sy71
title: 'Dual-view library: persistent Search and Explore views'
status: in-progress
type: feature
priority: normal
created_at: 2026-03-16T10:26:50Z
updated_at: 2026-03-16T16:08:24Z
---

Add two persistent, switchable views to the asset library:

**Search view** — the current search/filter experience. User searches for assets (e.g. SFX), sees results, can play/preview them.

**Explore view** — a directory tree browser. Shows the filesystem hierarchy of scanned folders, lets users navigate into directories and see what files are around a given asset. Useful when a search result looks promising and the user wants to explore nearby files in context.

The two views should be switchable (e.g. tabs), and both should be persistent — switching from Search to Explore and back doesn't lose your search query or your position in the tree.

A key workflow this enables: user searches for SFX, finds a good file, clicks to navigate to it in Explore view to see what other files are in that folder.


## Implementation Plan
- [x] Add `libraryView` state to ViewState (`search` | `explore`)
- [x] Create `explore.svelte.ts` state module (directory tree, selected path, cached children)
- [x] Add DB queries: `getDirectoryChildren()` and `getAssetsInDirectory()`
- [x] Create `DirectoryTree.svelte` recursive component
- [x] Create `ExploreView.svelte` (tree panel + content panel)
- [x] Add Search/Explore toggle to TabBar
- [x] Add "Show in Explore" action from search results (AudioList)
- [x] Fix `<svelte:self>` deprecation → self-import
- [x] All checks pass (svelte-check, vite build)
