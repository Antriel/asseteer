---
# asseteer-zvou
title: Speed up lazy thumbnail loading to match processing performance
status: completed
type: bug
priority: normal
created_at: 2026-03-23T15:01:27Z
updated_at: 2026-03-23T15:02:32Z
---

Library lazy-load thumbnail worker processes ZIP assets sequentially (1 at a time), while the Processing path spawns them in parallel. ZipCache already supports concurrent multi-slot reads with memory budget enforcement, but the thumbnail worker doesn't use this. Fix: spawn all assets (FS + ZIP) in parallel and increase batch size from 6 to 48.


## Summary of Changes

- Removed `MAX_CONCURRENT_FS = 3` semaphore and sequential ZIP loop from `thumbnail_worker.rs`
- Added `BATCH_SIZE = 48` (was `MAX_CONCURRENT_FS * 2 = 6`)
- All assets (FS + ZIP) now spawned as parallel tasks in one pass
- ZipCache's existing Condvar-based memory budget acts as the natural throttle for concurrent nested-ZIP loads
- Removed unused `HashMap`, `Arc`, and `resolve_zip_path` imports
