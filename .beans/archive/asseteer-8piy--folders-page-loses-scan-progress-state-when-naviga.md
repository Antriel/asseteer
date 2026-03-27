---
# asseteer-8piy
title: Folders page loses scan progress state when navigating away and back
status: completed
type: bug
priority: normal
created_at: 2026-03-22T07:57:42Z
updated_at: 2026-03-24T07:09:17Z
---

When scanning/importing a folder and navigating away from the folders page (e.g. to settings/library/processing), then returning to the folders page, it no longer shows scan progress. Instead it shows 0 assets and no scan history, despite scanning continuing in the background.

## Summary of Changes

Moved `scanProgress` from a local component variable to `uiState.scanProgress` (which already existed in `UIState` but was unused). Extracted scan progress update logic into `applyScanProgress()`. Added re-subscription to `scan-progress` events in `onMount` when `uiState.isScanning` is true.

When navigating away mid-scan, `uiState.isScanning` and `uiState.scanProgress` now persist in the global singleton. On return, the component detects the in-progress scan and re-subscribes to events, restoring the progress display immediately.


## Reopened: Additional symptom observed

During a large import, navigating back to Folders showed the import status — but as a *new card* appearing alongside an existing card for the same in-flight bundle that showed **assets: 0**. So the fix may be incomplete: the state is persisted, but a duplicate card is being rendered instead of updating the existing one.

## Fix for Reopened Symptom

Root cause: The backend inserts the folder record into `source_folders` immediately (with `asset_count = 0`) before scanning begins. When navigating back mid-scan, `loadFolders()` returns this partial record, creating a folder card showing "0 assets" alongside the scan progress banner.

Two changes:
1. Added `scanningFolderPath` to `uiState`. Set to the selected folder path when scanning starts, cleared on completion. The folders template filters out this folder while `isScanning` is true — so only the progress banner shows, not a duplicate card.
2. Added a `scan-complete` listener in `onMount`. When the scan finishes (from the originating component), this triggers `loadFolders()` on the current component so the newly completed folder appears with the correct asset count.
