---
# asseteer-tqt4
title: 'Folder management view: add ''Open in Explorer'' button/context menu'
status: completed
type: feature
priority: normal
created_at: 2026-03-20T08:26:19Z
updated_at: 2026-03-20T09:40:43Z
parent: asseteer-kvnt
---

In the folder management view (where users add/remove watched folders), add a way to open a folder in the system file explorer — either a button per row or a right-click context menu option.

## Summary of Changes

Added an "Open in File Explorer" button (external link icon) to the actions row for each folder in `src/routes/(app)/folders/+page.svelte`. Calls `openPath(folder.path)` directly since the full path is already available on `SourceFolder`.
