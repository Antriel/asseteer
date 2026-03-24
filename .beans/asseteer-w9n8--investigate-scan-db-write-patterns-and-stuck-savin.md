---
# asseteer-w9n8
title: Investigate scan DB write patterns and stuck Saving UI
status: completed
type: task
priority: normal
created_at: 2026-03-24T10:33:57Z
updated_at: 2026-03-24T10:47:32Z
---

Investigate why .db grows during scan instead of WAL, why WAL remains large after import, and why UI gets stuck on Saving message


## Investigation Findings

### 1. Why .db grows during scan (not WAL)

`PRAGMA wal_autocheckpoint=0` is per-connection. Two sources of unintended checkpointing:
- **Frontend `tauri-plugin-sql` connection** had default `wal_autocheckpoint=1000` — every frontend read triggered checkpoint when WAL exceeded 1000 pages (~4MB)
- **Backend sqlx pool** has 5 connections, but the PRAGMA only ran on whichever connection the pool dispatched it to — other 4 had default autocheckpoint

### 2. Why WAL remains large after import

`PRAGMA wal_checkpoint(PASSIVE)` skips pages locked by active readers. The massive FTS bulk insert created many dirty WAL pages, and PASSIVE couldn't checkpoint them all if the frontend held any shared lock.

### 3. "Saving..." UI stuck

After chunk insertion loop, three slow operations ran with **zero progress events**:
1. `populate_fts_for_new_assets` — two `INSERT INTO...SELECT` for ~940k rows each (trigram + unicode61 FTS5)
2. `PRAGMA wal_autocheckpoint=1000` restore
3. `PRAGMA wal_checkpoint(PASSIVE)`

UI showed stale last progress: "Saving... 939724/940354 (100%)"

### 4. Bottleneck

Single-writer SQLite on system drive. CPU/assets-drive idle because all work is sequential B-tree page splits + 4 index maintenance + FTS5 trigram tokenizer + WAL→.db checkpointing.

## Implementation

- [x] Set `wal_autocheckpoint=0` on all backend pool connections via `after_connect`
- [x] Set `wal_autocheckpoint=0` on frontend SQL plugin connection after load
- [x] Replace bulk FTS with batched version (50k rows/batch) with "indexing" phase progress events
- [x] Add "indexing" phase to frontend progress display (ScanControl + folders page)
- [x] Add passive WAL checkpoint after processing completes (work_queue.rs)
- [x] Remove per-scan autocheckpoint PRAGMA (redundant with after_connect)
- [x] Keep explicit PASSIVE checkpoint after scan

## Summary of Changes

**WAL checkpoint control**: Disabled auto-checkpoint on ALL connections (backend via `after_connect`, frontend after `Database.load()`). Checkpoints now happen only at explicit points: after scan FTS indexing, and after each processing category completes. Eliminates continuous .db writes during bulk operations.

**Batched FTS indexing with progress**: Replaced single massive `INSERT INTO...SELECT` with 50k-row batches, emitting "indexing" phase progress events. UI now shows "Indexing for search... X/Y (Z%)" instead of appearing stuck.

**Files changed**: `database/mod.rs`, `commands/scan.rs`, `task_system/work_queue.rs`, `database/connection.ts`, `ScanControl.svelte`, `ui.svelte.ts`, `folders/+page.svelte`
