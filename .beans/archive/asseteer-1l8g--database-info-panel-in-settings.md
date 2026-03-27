---
# asseteer-1l8g
title: Database info panel in Settings
status: completed
type: feature
priority: low
created_at: 2026-03-25T11:37:28Z
updated_at: 2026-03-25T11:52:49Z
---

Add a "Database" section to the Settings page with:

- **Open DB folder** button — opens the directory containing the SQLite file
- **DB size** — show current file size (main DB + WAL)
- **Stats** — anything interesting: total assets, total folders, free pages, page count, etc.
- **Vacuum button** — explicit manual VACUUM with progress/spinner. Important: temporarily enable WAL auto-checkpointing before running VACUUM to prevent duplicating the full DB size into the WAL file. Restore auto-checkpoint setting afterward.


## Summary of Changes

- **Backend**: Added `src-tauri/src/commands/database.rs` with two Tauri commands:
  - `get_db_info` — returns DB path, main+WAL file sizes, page count/size, freelist count, total assets, total folders
  - `vacuum_database` — runs VACUUM with temporary WAL auto-checkpoint adjustment to prevent WAL bloat, then truncates WAL
- **Frontend**: Added Database section to Settings page (`src/routes/(app)/settings/+page.svelte`) showing:
  - DB size (main + WAL breakdown)
  - Asset count, source folder count
  - Reclaimable space (shown only when freelist > 0)
  - "Open DB Folder" button
  - "Compact Database" button with spinner and confirmation dialog
- **Utility**: Extended `formatFileSize()` to handle GB range
- Replaced local `formatBytes()` in settings with shared `formatFileSize()` utility
