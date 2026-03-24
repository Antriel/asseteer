---
# asseteer-6o1x
title: Import warnings not surfaced to user (e.g. invalid ZIP archives)
status: completed
type: bug
priority: normal
created_at: 2026-03-23T15:43:31Z
updated_at: 2026-03-24T09:38:52Z
---

During import, warning messages are logged to the backend (e.g. 'Warning: Failed to read zip archive bundle.zip: invalid Zip archive: Could not find EOCD') but never shown to the user. These should be collected and surfaced — either as toast notifications or in a warnings section on the scan result card — so users know which files failed and why.


## Summary of Changes

- Added `warnings: Vec<String>` field to `ScanProgress` struct (emitted in `complete` event only)
- Added `warnings: Mutex<Vec<String>>` to `ScanProgressState` for thread-safe collection across rayon tasks
- Replaced all `eprintln!("Warning: ...")` calls in `discover_zip_parallel`, `discover_nested_zip_parallel`, and the main walk with both eprintln (full path, for debugging) and a push to `progress.warnings` (filename + error, for user display)
- `add_folder`: collects warnings after discovery and includes them in the `scan-progress` `complete` event
- `RescanPreviewResult` gains a `warnings` field; `preview_rescan` collects and returns them
- Frontend `ScanProgressEvent` and `RescanPreviewResult` interfaces updated with optional `warnings` field
- `applyScanProgress`: shows a warning toast on scan completion when warnings exist (1 warning → full message; N warnings → count summary), with 8s duration
- `startRescan`: shows warning toast after `preview_rescan` invoke if warnings were collected


## Follow-up Changes (user feedback)

**Bug fix**: Folder card not appearing after scan
- Restructured `addFolder` to call `loadFolders()` after the `finally` block (after `isScanning = false`), eliminating the race where the new folder was filtered out of the list while scanning was still flagged as active.

**Persistent warnings in folder card**
- Added `scan_warnings TEXT` column to `source_folders` table via `ALTER TABLE` migration (run at startup, error ignored if already present)
- Added `scan_warnings: Option<String>` to `SourceFolder` Rust model (JSON-encoded array)
- `add_folder`: saves warnings as JSON to `source_folders.scan_warnings` on completion (NULL if none)
- `apply_rescan`: carries warnings from `CachedRescanPreview` and saves to DB on apply
- `list_folders` now includes `scan_warnings` in the SELECT
- Frontend `SourceFolder` type updated with `scan_warnings: string | null`
- Folder card shows a persistent warning section (amber box with per-file list) when `scan_warnings` is present; cleared on next successful scan with no warnings
