---
# asseteer-swe5
title: START ALL parallel processing causes resource contention and RAM issues
status: todo
type: bug
priority: normal
created_at: 2026-03-26T14:50:28Z
updated_at: 2026-03-26T16:04:44Z
---

When using START ALL, processing categories run in parallel. All categories compete for the same resources: disk I/O (ZIP extraction), RAM (preloaded bytes, zip_cache), and CPU. CLAP additionally competes for GPU. Running them simultaneously causes unpredictable throughput drops and RAM spikes.

## Current state (post asseteer-mmwn)

CLAP bulk ZIP extraction (asseteer-mmwn) reduced per-batch prep from ~6-9s to <1s, which lessens but doesn't eliminate contention. The new bulk extraction path (`bulk_load_from_zip`) loads bytes outside zip_cache with no memory budget — 3 concurrent CLAP batches × 32 assets × ~1-5MB = ~100-500MB unbudgeted. The existing zip_cache memory budgeting only covers nested ZIPs.

## Proposed fix: sequential category processing

Make categories run one at a time rather than in parallel. They all fight over the same resources (disk I/O, ZIP handles, RAM), and running sequentially gives each category full throughput without contention.

Implementation approach:
- Add a global processing lock (e.g. `Mutex` or `AtomicBool` on `WorkQueue`) so only one category processes at a time
- "START ALL" queues categories sequentially (e.g. Image → Audio → CLAP) rather than launching all at once
- Frontend shows queued categories as "waiting" with their position
- Each category gets full access to disk/RAM/CPU during its turn, then cleans up (evict_unpinned, etc.) before the next starts

This is simpler and more predictable than trying to budget shared resources across concurrent categories.

## Tasks
- [ ] Add global processing lock so only one category runs at a time
- [ ] Change START ALL to queue categories sequentially
- [ ] Update frontend to show queued/waiting state for pending categories
- [ ] Verify RAM stays bounded and throughput is stable per-category
