---
# asseteer-y7eo
title: 'Library folders view: right-click context menu to open folder in system explorer'
status: completed
type: feature
priority: normal
created_at: 2026-03-20T08:26:16Z
updated_at: 2026-03-20T09:40:43Z
parent: asseteer-kvnt
---

Add a right-click context menu to the folders list in the library view. Menu should have an 'Open in Explorer' option that opens the selected folder in the system file explorer. We already do this for audio files, so reuse that pattern.

## Summary of Changes

- `explore.svelte.ts`: Added `folderPaths: Map<number, string>` populated at root load time, mapping folderId to absolute folder path.
- `DirectoryTree.svelte`: Added right-click context menu per row with "Open in File Explorer", computing the absolute path from `exploreState.folderPaths` + the node's `relPath`.
