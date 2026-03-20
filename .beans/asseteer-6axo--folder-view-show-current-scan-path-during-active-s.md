---
# asseteer-6axo
title: 'Folder view: show current scan path during active scan'
status: completed
type: feature
priority: normal
created_at: 2026-03-19T11:41:04Z
updated_at: 2026-03-20T07:24:51Z
parent: asseteer-kvnt
---

While a folder is being scanned, show at minimum the root folder path being scanned. Live-updating the current file being processed would be ideal if trivial to add without harming scan performance — but static display of the root is acceptable. Improves user confidence that something is happening during long scans.


## Summary of Changes

- Populated `uiState.scanDetails` from the `scan-progress` event listener in `addFolder()`, so the StatusBar now correctly shows phase/count detail during scanning (it was reading this data but nothing ever wrote to it).
- Added a second line to the folders-page scan banner that shows `currentPath` (truncated, with full path as tooltip) when the backend reports one, giving live feedback on the file currently being scanned.
