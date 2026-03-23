---
# asseteer-8piy
title: Folders page loses scan progress state when navigating away and back
status: completed
type: bug
priority: normal
created_at: 2026-03-22T07:57:42Z
updated_at: 2026-03-23T14:35:35Z
---

When scanning/importing a folder and navigating away from the folders page (e.g. to settings/library/processing), then returning to the folders page, it no longer shows scan progress. Instead it shows 0 assets and no scan history, despite scanning continuing in the background.

## Summary of Changes

Moved `scanProgress` from a local component variable to `uiState.scanProgress` (which already existed in `UIState` but was unused). Extracted scan progress update logic into `applyScanProgress()`. Added re-subscription to `scan-progress` events in `onMount` when `uiState.isScanning` is true.

When navigating away mid-scan, `uiState.isScanning` and `uiState.scanProgress` now persist in the global singleton. On return, the component detects the in-progress scan and re-subscribes to events, restoring the progress display immediately.
