---
# asseteer-wxak
title: 'Folder management: add, remove, and rescan source folders'
status: draft
type: feature
priority: normal
created_at: 2026-03-16T11:00:19Z
updated_at: 2026-03-16T11:02:13Z
blocked_by:
    - asseteer-n0v9
---

Complete folder management system so users can view their scanned folders, rescan them to pick up new/deleted files, and remove folders (with their assets) from the library.

Currently: users can scan a folder once via /scan, but there's no persistent folder concept. scan_sessions tracks scan history but isn't linked to assets and has no management UI. There's no way to rescan a folder for changes or remove a folder's assets from the library.


## Current State

- **Scanning works** — user picks a folder, `start_scan` discovers files recursively (including inside ZIPs), inserts them into `assets` table
- **No persistent folder concept** — `scan_sessions` records scan history but isn't linked to assets; the `assets.path` field implicitly records the source folder
- **No rescan** — no command or UI to re-scan a previously scanned folder for new/removed files
- **No folder removal** — no way to unlink a folder and cascade-delete its assets + metadata
- **No folder list UI** — no page or sidebar section showing managed folders

## Architecture Plan

### Phase 1: Database — `source_folders` table

Add a new `source_folders` table as the core concept:

```sql
CREATE TABLE source_folders (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,          -- absolute filesystem path
    label TEXT,                         -- optional user-friendly name
    added_at INTEGER NOT NULL,          -- unix timestamp
    last_scanned_at INTEGER,            -- unix timestamp of last completed scan
    asset_count INTEGER DEFAULT 0,      -- denormalized count for UI
    status TEXT DEFAULT 'active'        -- 'active' | 'removed'
);
```

Add `source_folder_id INTEGER REFERENCES source_folders(id)` column to `assets` table, backfill from `assets.path`.

Update `scan_sessions` to reference `source_folder_id` instead of raw `root_path`.

### Phase 2: Rust backend commands

**New commands:**

- `add_folder(path)` — insert into `source_folders`, then run scan (reuse existing `discover_files` + `insert_pending_assets`)
- `rescan_folder(folder_id)` — diff filesystem vs. database:
  - Discover files on disk (existing `discover_files` logic)
  - Compare against `assets` WHERE `source_folder_id = ?`
  - **New files** → insert into `assets` (status: pending processing)
  - **Deleted files** → mark assets as missing or delete them + cascade metadata
  - **Unchanged files** → skip
  - Update `last_scanned_at` and `asset_count`
- `remove_folder(folder_id)` — cascade delete: assets → image_metadata, audio_metadata, audio_embeddings, processing_errors; then delete or mark folder as 'removed'
- `list_folders()` — return all active source folders with asset counts and last-scanned timestamps
- `rename_folder(folder_id, label)` — update user-friendly label

**Refactor `start_scan`** to go through `add_folder` flow so every scan creates/reuses a `source_folders` record.

### Phase 3: Frontend — Folder management UI

**Option A (sidebar section):** Add a "Folders" section to the sidebar listing managed folders with status indicators and action buttons.

**Option B (dedicated route):** `/folders` page with a table/list of managed folders, each showing:
- Folder path + label
- Asset count (images / audio breakdown)
- Last scanned timestamp
- Status (scanning, up-to-date, has changes)
- Actions: Rescan, Remove, Rename

**Recommended: Option A + B hybrid** — sidebar shows folders as a compact list (click to filter library by folder), `/folders` page for full management.

### Phase 4: Rescan UX

- "Rescan" button per folder in the management UI
- Show diff summary before applying: "Found 12 new files, 3 removed files"
- Progress events reuse existing `scan-progress` pattern
- After rescan, auto-trigger processing for newly added assets (or prompt user)
- Consider: file watcher for auto-detection of changes (future enhancement, not MVP)

### Phase 5: Folder removal UX

- "Remove" button with confirmation dialog (using `showConfirm`)
- Show what will be deleted: "Remove folder and 847 assets?"
- Cascade delete all related data (metadata, embeddings, errors)
- Consider "soft remove" (hide from library but keep data) vs. hard delete

## Implementation Tasks

- [ ] Create `source_folders` table (migration)
- [ ] Add `source_folder_id` FK to `assets` table (migration + backfill)
- [ ] Update `scan_sessions` to reference `source_folder_id`
- [ ] Implement `add_folder` command (wraps existing scan logic)
- [ ] Implement `list_folders` command
- [ ] Implement `rescan_folder` command (diff-based)
- [ ] Implement `remove_folder` command (cascade delete)
- [ ] Refactor `start_scan` to use `source_folders`
- [ ] Build folder list UI (sidebar + management page)
- [ ] Build rescan UX (button, progress, diff summary)
- [ ] Build remove UX (confirmation, cascade feedback)
- [ ] Add "filter library by folder" support
- [ ] Update /scan page to integrate with folder management flow

## Edge Cases to Handle

- Folder path no longer exists on disk (moved/deleted externally)
- Folder is a subdirectory of an already-managed folder (overlap detection)
- Network/removable drives that may be intermittently available
- Very large folders (100k+ files) — streaming/batched rescan
- ZIP files that changed on disk (re-extract and diff)
- Assets referenced by both a folder scan and a ZIP inside that folder

## Dependencies

- Related to `asseteer-j66e` (scan pipeline blocking/memory issues) — rescan should use streaming approach
- Related to `asseteer-n0v9` (scan page listener leak) — should be fixed before adding more scan UI
