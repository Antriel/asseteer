---
# asseteer-4v5r
title: 'Unified library: replace Search/Explore toggle with collapsible folder sidebar'
status: completed
type: feature
priority: normal
created_at: 2026-03-17T09:04:31Z
updated_at: 2026-03-17T09:30:08Z
---

Replace the separate Search/Explore modes with a unified library view where the folder tree is a collapsible sidebar that acts as a filter on the main content. Search and folder selection become combinable filters. Add 'Show in folder' action on search results, breadcrumb bar for folder navigation, and recursive folder display.


## Summary of Changes

Replaced the separate Search/Explore toggle with a unified library view:

**Data layer (`queries.ts`)**:
- Added `folderPath` parameter to `searchAssets()` and `countSearchResults()` for recursive folder filtering via `(path = ? OR path LIKE ?)` 

**State (`assets.svelte.ts`, `view.svelte.ts`, `explore.svelte.ts`)**:
- Added `folderPath` filter to `AssetsState` with `setFolderFilter()` method
- Assets now load when folder is selected (even without search text)
- Replaced `libraryView: 'search' | 'explore'` with `folderSidebarOpen: boolean`
- Simplified `ExploreState` to tree-only state (removed asset loading)

**UI Components**:
- `TabBar`: Replaced Search/Explore segmented control with Folders toggle button
- `Toolbar`: Added folder breadcrumb bar with clear button when folder filter active
- `Toolbar`: Search input now initializes from persistent `assetsState.searchText` (fixes bug #1)
- Created `FolderSidebar.svelte` — collapsible folder tree panel
- `DirectoryTree`: Now accepts `onSelect` callback instead of directly loading assets
- `AudioList`: "Show in Explore" → "Show in folder" (opens sidebar + sets filter)
- Deleted `ExploreView.svelte` (replaced by unified layout)

**Library page (`+page.svelte`)**:
- Single unified layout: TabBar → Toolbar → [Sidebar | Content]
- Content area always uses same components (ImageGrid/AudioList/AssetList)
- Fixed onMount to respect active tab (fixes bug #2)
- Improved empty states for folder-only, search-only, and combined filters

**Bug fixes included**:
1. Search string now persists when navigating away and back
2. Audio tab selection now persists when navigating away and back
3. Folder browsing now shows recursive contents (fixes "No assets" bug)


## Additional Changes (follow-up)

**"Show in folder" for images:**
- Added to `ImageLightbox.svelte`: folder icon button in the controls bar, closes lightbox and navigates to the asset's folder
- Both lightbox and AudioList `showInFolder` handle ZIP entries: navigates to the ZIP file in the tree, sets folder filter with `::` encoding

**ZIP folder browsing:**
- `DirectoryNode` extended with optional `zipPrefix` field
- `buildChildNodes` detects ZIP files (paths with `zip_entry` values) and marks them as expandable (`childCount > 0`)
- New `getZipDirectoryChildren(db, zipPath, prefix)` query parses `zip_entry` paths to build internal directory structure
- `explore.svelte.ts` `loadChildren` detects ZIP paths (`.zip` extension or `::` separator) and calls appropriate query
- `DirectoryTree` shows a different icon for ZIP nodes (cube/archive icon vs folder)
- `FolderSidebar.selectFolder` encodes ZIP paths with `::` separator for the folder filter
- `addFolderFilterConditions` handles ZIP-internal filtering: `path = ? AND zip_entry LIKE ?`
- `Toolbar` breadcrumb displays ZIP paths nicely (e.g., "archive.zip / textures")
- Nested ZIPs work naturally since `zip_entry` contains the full traversal path (e.g., `nested.zip/subfolder/file.jpg`)
