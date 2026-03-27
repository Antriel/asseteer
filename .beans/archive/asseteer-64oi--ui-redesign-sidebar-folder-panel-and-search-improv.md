---
# asseteer-64oi
title: 'UI redesign: sidebar, folder panel, and search improvements'
status: completed
type: feature
priority: normal
created_at: 2026-03-18T09:12:45Z
updated_at: 2026-03-19T10:39:50Z
---

Redesign the left panel area for better UX:

1. Collapsible sidebar (icons-only vs full)
2. Move folder panel from inside library to root layout (between sidebar and content)
3. Resizable folder panel width (drag handle, 200-500px range)
4. Fix folder tree: only expand on chevron click, not name click
5. Faster folder root loading (show roots immediately, load counts async)
6. Better empty state when search + folder filter yields no results
7. Clear button on search input

## Tasks
- [x] Add sidebarCollapsed state to viewState
- [x] Update Sidebar.svelte for collapsed/expanded modes with toggle
- [x] Move FolderSidebar rendering to root layout
- [x] Add resize handle to FolderSidebar
- [x] Store folder panel width in viewState
- [x] Fix DirectoryTree click: separate chevron from row click
- [x] Async folder root count loading
- [x] Improve empty state messaging with clear actions
- [x] Add clear (X) button to search input


## Known issue: folder tree still broken for ZIPs

The `buildChildNodes` function cannot reliably distinguish files from directories because `assets.path` stores the full file path (including filename). ZIP files look identical to regular files at the same path depth, so heuristics to skip files also skip ZIPs. The proper fix is the schema redesign in asseteer-zmc8 (folder_id + rel_path), which would make directory structure explicit rather than inferred from file paths.

## Summary of Changes

All tasks were already complete. The known ZIP folder tree issue was resolved by the schema redesign in asseteer-wxak (folder_id + rel_path + zip_file separation).
