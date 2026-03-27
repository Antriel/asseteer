---
# asseteer-3n6g
title: Investigate batch DB writes for import stage (similar to asseteer-r0uy)
status: completed
type: task
priority: normal
created_at: 2026-03-23T15:43:25Z
updated_at: 2026-03-24T08:04:05Z
---

During a large import, high drive write rates were observed. The batch DB write optimization from asseteer-r0uy was applied to image/audio processing workers, but the import/scan stage may still be doing per-item writes. Investigate whether the same batching approach should be applied to the import pipeline to reduce write overhead and drive activity.


## Investigation Findings

The import stage **already batches** 200 assets/transaction via `insert_asset_chunk`. The high system-drive write rate is caused by **write amplification**:

1. **FTS5 dual-index triggers** — Each `INSERT INTO assets` fires `assets_ai` trigger → 2 additional FTS inserts (trigram + unicode61). 3x writes per asset.
2. **4 B-tree indexes** maintained per INSERT (type, folder, modified, 4-column unique composite)
3. **WAL checkpoint overhead** — continuous checkpointing during sustained writes

## Implementation Plan

- [x] Remove `assets_ai` (INSERT trigger) permanently; keep UPDATE/DELETE triggers
- [x] Add migration: `DROP TRIGGER IF EXISTS assets_ai` for existing DBs
- [x] `scan.rs`: increase `CHUNK_SIZE` 200→1000
- [x] `scan.rs`: suppress WAL autocheckpoint during scan, checkpoint after
- [x] `scan.rs`: bulk-populate FTS after all chunks (folder_id + max_id scoped)
- [x] `rescan.rs`: add inline FTS population for new assets in transaction
- [x] Verify concurrent scan safety (folder_id scoping prevents cross-scan FTS conflicts)


## Summary of Changes

Three optimizations to reduce system-drive write I/O during large imports:

1. **Removed FTS INSERT trigger** (`assets_ai`) permanently from schema. Both FTS indexes (trigram + unicode61) were being populated per-row via trigger, tripling effective writes. Now bulk-populated explicitly after scan/rescan with `INSERT INTO ... SELECT` queries scoped by `folder_id + max_id` for concurrent scan safety. UPDATE/DELETE triggers kept for convenience.

2. **Increased chunk size** from 200 to 1000 assets per transaction, reducing transaction overhead for the pure-insert workload.

3. **WAL autocheckpoint suppressed** during scan (`PRAGMA wal_autocheckpoint=0`), restored after with a single passive checkpoint. Prevents continuous checkpoint I/O during sustained writes.

Files changed: `schema.rs`, `init.rs`, `scan.rs`, `rescan.rs`, `concurrent_tests.rs`
All 66 tests pass.
