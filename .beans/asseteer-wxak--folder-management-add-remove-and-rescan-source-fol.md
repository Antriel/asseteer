---
# asseteer-wxak
title: 'Folder management: add, remove, and rescan source folders'
status: in-progress
type: feature
priority: normal
created_at: 2026-03-16T11:00:19Z
updated_at: 2026-03-19T10:21:29Z
---

Complete folder management system: persistent source folders, relative paths, add/remove/rescan, and management UI.

**No migration needed** — the DB schema is designed from scratch. Existing databases will be wiped and rescanned.

This bean absorbs asseteer-zmc8 (relative paths) and asseteer-1r6b (path normalization) since without migration there's no reason to phase these separately. All paths normalized to forward slashes at write time.

## Current State

- **Scanning works** — user picks a folder, `start_scan` discovers files recursively (including inside ZIPs), inserts them into `assets` table
- **No persistent folder concept** — `scan_sessions` records scan history but isn't linked to assets; the `assets.path` field stores full absolute paths
- **No rescan** — no way to re-scan a previously scanned folder for new/removed files
- **No folder removal** — no way to unlink a folder and cascade-delete its assets + metadata
- **No folder list UI** — no page or sidebar section showing managed folders

## Database Schema

### `source_folders` table

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

### `assets` table changes

Replace `assets.path` (absolute) with `folder_id` + `rel_path` + `zip_file`:

```sql
CREATE TABLE assets (
    ...
    folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    rel_path TEXT NOT NULL,    -- relative directory path within folder (no filename, forward slashes)
    filename TEXT NOT NULL,    -- bare filename of the actual asset
    zip_file TEXT,             -- ZIP filename if asset is inside a ZIP (NULL for filesystem files)
    zip_entry TEXT,            -- full path inside ZIP including filename (NULL for filesystem files)
    file_size INTEGER,         -- filesystem file size in bytes (for regular files); ZIP archive size (for ZIP assets)
    fs_modified_at INTEGER,    -- filesystem mtime as unix timestamp (for change detection during rescan)
    ...
);
CREATE UNIQUE INDEX idx_assets_unique ON assets(
    folder_id, rel_path, COALESCE(zip_file, ''), COALESCE(zip_entry, filename)
);
CREATE INDEX idx_assets_folder ON assets(folder_id);
```

**Important**: `fs_modified_at` must store the actual filesystem mtime, NOT the scan timestamp. The current code writes `modified_at` as scan time — this must be fixed. For ZIP assets, store the mtime of the outer ZIP file (if the ZIP changed, all its contents need re-scanning).

All paths normalized to forward slashes at write time.

`rel_path` is **always** the filesystem directory portion (no filename, no ZIP name). This is consistent for both regular files and ZIP assets — it's the directory that contains either the file or the ZIP archive on the filesystem.

#### Storage examples

```
Source folder: D:\Assets

Regular file:   D:\Assets\Packs\textures\grass.png
  folder_id=1  rel_path="Packs/textures"  filename="grass.png"  zip_file=NULL          zip_entry=NULL

File in ZIP:    D:\Assets\Packs\sounds.zip → ambient/forest_01.wav
  folder_id=1  rel_path="Packs"           filename="forest_01.wav" zip_file="sounds.zip" zip_entry="ambient/forest_01.wav"

Nested ZIP:     D:\Assets\Packs\sounds.zip → inner.zip/subfolder/deep.wav
  folder_id=1  rel_path="Packs"           filename="deep.wav"    zip_file="sounds.zip"  zip_entry="inner.zip/subfolder/deep.wav"

Deep nested:    D:\Assets\Packs\sounds.zip → folder/inner.zip/subfolder/deep.wav
  folder_id=1  rel_path="Packs"           filename="deep.wav"    zip_file="sounds.zip"  zip_entry="folder/inner.zip/subfolder/deep.wav"
```

#### Path reconstruction

- **Regular file**: `source_folders.path + '/' + rel_path + '/' + filename`
- **ZIP asset**: open `source_folders.path + '/' + rel_path + '/' + zip_file` → extract `zip_entry`
- **Nested ZIP**: same as ZIP — the nesting chain is encoded in `zip_entry` as today (e.g., `inner.zip/subfolder/deep.wav`)

#### Why `zip_file` is needed

Previously, `assets.path` pointed to the ZIP file on disk, so the ZIP filename was implicit. With `folder_id + rel_path`, we lose that. `zip_file` explicitly identifies which ZIP archive in the directory contains the asset. Without it, there'd be no way to distinguish "filesystem file `forest_01.wav` in `Packs/`" from "file `forest_01.wav` inside `Packs/sounds.zip`".

#### Folder tree queries

- **Filesystem directories**: `SELECT DISTINCT rel_path FROM assets WHERE folder_id = ? AND zip_file IS NULL` → `GROUP BY` gives clean directory nodes
- **ZIP files in a directory**: `SELECT DISTINCT zip_file FROM assets WHERE folder_id = ? AND rel_path = ? AND zip_file IS NOT NULL` → shows ZIP archives as expandable nodes
- **Inside a ZIP**: parse directory prefixes from `zip_entry` (same approach as current `getZipDirectoryChildren`, but querying on `folder_id + rel_path + zip_file` instead of `path`)
- This eliminates the current folder-tree bugs where files and ZIP archives are confused with directories (since `rel_path` is always a filesystem directory, never a filename)

### `scan_sessions` changes

Reference `source_folder_id` instead of raw `root_path`.

### `folder_search_config` table

Per-subfolder control over which path segments are indexed for search (see asseteer-1vw2 for full design):

```sql
CREATE TABLE folder_search_config (
    id INTEGER PRIMARY KEY,
    source_folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    subfolder_prefix TEXT NOT NULL DEFAULT '',
    skip_depth INTEGER NOT NULL DEFAULT 0,
    UNIQUE(source_folder_id, subfolder_prefix)
);
```

This table is created here but primarily used by the FTS rework in asseteer-1vw2. Include it in the schema so the FTS triggers can reference it from day one.

## Rust Backend Commands

**New commands:**

- `add_folder(path)` — insert into `source_folders`, then run scan. Computes `rel_path` (directory portion relative to source folder) and `filename` for each discovered file.
- `preview_rescan(folder_id)` — dry-run diff, returns summary without mutating:
  - Discover files on disk (existing `discover_files` logic)
  - Compare against `assets` WHERE `folder_id = ?`
  - **Change detection**: compare `file_size` + `fs_modified_at` against filesystem stat. If either differs → modified.
  - For ZIP files: if the ZIP's size/mtime changed, all assets inside it are marked as potentially modified and the ZIP is re-scanned for added/removed entries.
  - Returns: `{ added: [...], removed: [...], modified: [...], unchanged: count }`
- `apply_rescan(folder_id, preview_id)` — applies a previously computed diff:
  - **New files** → insert into `assets` (status: pending processing)
  - **Deleted files** → delete assets (CASCADE handles metadata cleanup)
  - **Modified files** → update `file_size` + `fs_modified_at`, delete derived data (`image_metadata`, `audio_metadata`, `audio_embeddings` rows for that asset) so the processing pipeline sees them as unprocessed, then re-queue for processing
  - **Unchanged files** → skip
  - Update `last_scanned_at` and `asset_count`
- `remove_folder(folder_id)` — delete from `source_folders` WHERE `id = ?`. `ON DELETE CASCADE` handles all asset + metadata cleanup automatically.
- `list_folders()` — return all active source folders with asset counts and last-scanned timestamps
- `rename_folder(folder_id, label)` — update user-friendly label
- `update_search_config(folder_id, subfolder_prefix, skip_depth)` — update `folder_search_config` and trigger FTS re-index for affected assets

**Refactor `start_scan`** to go through `add_folder` flow. Every scan creates/reuses a `source_folders` record. The scan pipeline must:
- Compute `rel_path` (directory portion, forward slashes) and `filename` from the absolute discovered path minus the source folder root
- For ZIP assets: set `zip_file` to the ZIP archive's filename, `rel_path` to the directory containing the ZIP on the filesystem, `filename` to the actual asset filename inside the ZIP, and `zip_entry` to the full internal path (unchanged from current behavior)
- For nested ZIPs: `zip_file` is always the outermost ZIP filename, `zip_entry` contains the nesting chain (e.g., `inner.zip/subfolder/deep.wav`) — same as current behavior
- `load_asset_bytes` / `get_asset_bytes` must be updated to reconstruct the ZIP path from `source_folders.path + rel_path + zip_file` instead of using `assets.path` directly

## Navigation Contract: Replacing the `path::zip_prefix` String

The current codebase threads a single path string (with `::` ZIP encoding) through many layers: folder filter state, URL params, tree selection, context menus, folder filter parsing in queries, etc. This is a significant refactor.

**Replace with a `FolderLocation` type** (TypeScript + Rust equivalent):

```typescript
type FolderLocation =
  | { type: 'folder'; folderId: number; relPath: string }
  | { type: 'zip'; folderId: number; relPath: string; zipFile: string; zipPrefix: string }
```

This replaces `folderPath: string | null` everywhere it appears:
- `assets.svelte.ts` — `folderPath` state → `folderLocation: FolderLocation | null`
- `explore.svelte.ts` — tree node selection, path encoding
- `queries.ts` — `parseFolderFilter()`, `addFolderFilterConditions()` → accept `FolderLocation`
- `Toolbar.svelte` — any folder display
- URL/route params if folder selection is persisted in the URL
- Context menu "show in folder" actions — reconstruct full path from `FolderLocation`

This is not just a query change — it touches state management, UI components, and the tree navigation model. Budget accordingly.

## Frontend — Folder Management UI

**Sidebar section:** "Folders" section listing managed folders as a compact list. Click to filter library by folder.

**Dedicated `/folders` page** for full management, each folder showing:
- Folder path + label
- Asset count (images / audio breakdown)
- Last scanned timestamp
- Status (scanning, up-to-date, has changes)
- Actions: Rescan, Remove, Rename
- Expandable subfolder tree with search depth controls (for `folder_search_config`)

### Rescan UX (two-phase)

- "Rescan" button per folder → calls `preview_rescan`, shows progress while scanning filesystem
- Preview dialog: "Found 12 new files, 3 removed, 5 modified" with option to expand details
- "Apply" button → calls `apply_rescan` to commit changes
- Progress events reuse existing `scan-progress` pattern
- After apply, auto-trigger processing for new/modified assets (or prompt user)

### Folder removal UX

- "Remove" button with confirmation dialog (using `showConfirm`)
- Show what will be deleted: "Remove folder and 847 assets?"
- Deletion is clean — `ON DELETE CASCADE` handles everything

## Implementation Tasks

### Schema & core backend
- [x] Design and create new DB schema (source_folders, updated assets with `file_size`/`fs_modified_at`, folder_search_config, scan_sessions)
- [x] Update `start_scan` / `add_folder` to compute `rel_path` + `filename` + `zip_file` from discovered absolute paths (normalize to forward slashes)
- [x] Store actual filesystem mtime in `fs_modified_at` (not scan time) and `file_size` during scan
- [x] Implement `list_folders` command
- [x] Implement `preview_rescan` command (dry-run diff using `file_size` + `fs_modified_at` comparison)
- [x] Implement `apply_rescan` command (commit previewed changes)
- [x] Implement `remove_folder` command (just DELETE, cascade does the rest)
- [x] Implement `rename_folder` command
- [ ] Implement `update_search_config` command + FTS re-index logic

### Path-dependent surfaces to update (schema swap)
All code that references `assets.path` or the `path::zip_prefix` convention must be updated:
- [x] `load_asset_bytes` / `get_asset_bytes` — reconstruct filesystem path from `source_folders.path + rel_path + filename/zip_file` (`utils.rs`, `commands/assets.rs`)
- [x] FTS triggers — currently index `assets.path`; must use `rel_path` + `filename` + `zip_entry` (`database/schema.rs`)
- [x] Semantic search commands — return `path` in payloads; must reconstruct or return new fields (`commands/search.rs`)
- [x] CLAP batching/sorting — groups by path for ZIP cache optimization; must use `folder_id + rel_path + zip_file` (`task_system/processor.rs`)
- [x] Thumbnail worker — ZIP grouping for batch extraction (`thumbnail_worker.rs`)
- [x] Processing error payloads — store/display `path` in error records (`commands/process.rs`)
- [x] Frontend queries — `searchAssets`, `countSearchResults`, `parseFolderFilter`, `addFolderFilterConditions` (`database/queries.ts`)
- [x] Frontend state — `folderPath` → `FolderLocation` type (`assets.svelte.ts`, `explore.svelte.ts`)
- [x] Folder tree building — `buildChildNodes`, `getZipDirectoryChildren` → new schema queries (`explore.svelte.ts`)
- [x] UI displays — anywhere that shows file path/location to the user (AudioList, ImageGrid, context menus, lightbox)
- [x] "Show in folder" / "Open in explorer" actions — reconstruct full filesystem path

### Frontend — folder management UI
- [x] Define `FolderLocation` type and replace `folderPath: string | null` throughout
- [x] Build folder list UI (sidebar section)
- [x] Build folder management page (`/folders` route)
- [x] Build rescan UX (two-phase: preview → confirm → apply)
- [x] Build remove UX (confirmation dialog)
- [x] Add "filter library by folder" support
- [x] Update /scan page to integrate with folder management flow

## Edge Cases to Handle

- Folder path no longer exists on disk (moved/deleted externally)
- Folder is a subdirectory of an already-managed folder (overlap detection)
- Network/removable drives that may be intermittently available
- Very large folders (100k+ files) — streaming/batched scan and rescan
- ZIP files that changed on disk (re-extract and diff)
- Assets referenced by both a folder scan and a ZIP inside that folder

## Downstream Dependents

- **asseteer-1vw2** (FTS rework) depends on this for `source_folders`, `folder_search_config`, and the `folder_id + rel_path + zip_file` schema. The FTS triggers need `rel_path`, `filename`, and `zip_entry` as separate columns, and need `folder_search_config` to compute `searchable_path`. For ZIP assets, `searchable_path` should include the directory portion of `zip_entry` (ZIP-internal directory structure is always searchable, not affected by search depth config which targets filesystem organizational noise only).
- **Folder tree browsing** is fixed as a side effect — `rel_path` (filesystem directory only) + `GROUP BY` gives correct directory nodes, `DISTINCT zip_file` shows ZIP archives as expandable nodes. No more confusion between files and directories.

## Absorbed beans

- **asseteer-zmc8** (relative paths) — full schema included here
- **asseteer-1r6b** (path normalization) — forward-slash normalization at write time included here
