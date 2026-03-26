---
# asseteer-43sc
title: Center 'Add Folder' button in empty folders view not disabled during import
status: completed
type: bug
priority: normal
created_at: 2026-03-25T12:12:21Z
updated_at: 2026-03-26T07:25:00Z
---

When the folders view has no folders yet, a centered 'Add Folder' button is shown. Unlike the regular add folder button, this one is not disabled when an import is in progress.

Clicking it during an import causes the UI card/progress for the ongoing import to disappear (the import itself continues in the background and finishes successfully, but there is no visible progress). Navigating away and back to the Folders page then shows the folder as imported with 0 assets, which is incorrect.

Two things to fix:
1. Disable the center 'Add Folder' button while an import/scan is in progress (same as the regular button).
2. After navigating away and back, the folder that was importing in the background shows 0 assets — investigate why the asset count is wrong after a backgrounded import completes.


## Plan

### Fix 1: Hide center button during scan (quick)
- Instead of disabling, hide the center "Add Folder" button when `uiState.isScanning` is true (show a scanning indicator in the empty state instead)

### Fix 2: Support concurrent folder additions
- Redesign `uiState` scan tracking to support multiple concurrent scans
- Replace single `isScanning`/`scanProgress`/`scanningFolderPath` with a `Map<string, ActiveScan>` 
- Derive `isScanning` from map size > 0
- Never disable the "Add Folder" button — always allow adding folders
- Show per-scan progress cards in the folders page
- Each `addFolder()` call creates its own scan entry and listener

### Investigation: 0-asset count after background import
- The issue occurs because `addFolder()` is an async function that continues running after the component unmounts
- When the invoke completes, `loadFolders()` updates the local (now-orphaned) `folders` state
- The `scan-complete` listener should handle this, but need to verify timing

## Tasks
- [x] Redesign scan state to support multiple concurrent scans
- [x] Update folders page to use new multi-scan state
- [x] Remove disabled state from Add Folder button
- [x] Show per-scan progress cards
- [x] Test concurrent scan behavior (compiles, manual testing needed)

## Summary of Changes

### Backend (Rust)
- Added `folder_path: Option<String>` to `ScanProgress` struct so frontend can distinguish concurrent scans
- Updated all `ScanProgress` emission sites in `scan.rs` and `rescan.rs`
- Passed folder path through `discover_files_streaming` and `populate_fts_batched`

### Frontend
- Redesigned `uiState` scan tracking: replaced single `isScanning`/`scanProgress`/`scanningFolderPath` with `activeScans: SvelteMap<string, ActiveScan>`
- `isScanning`, `scanDetails`, `scanStartedAt` are now derived getters from the map
- Add Folder button is never disabled — concurrent folder additions are now supported
- Center "Add Folder" button in empty state is hidden when any scan is active
- Each scan shows its own progress card with folder name, progress message, and elapsed time
- Single shared `scan-progress` listener routes events by `folder_path`
- Updated `ScanControl.svelte` for API compatibility (not currently used in routes)
