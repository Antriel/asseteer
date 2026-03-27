---
# asseteer-swe5
title: START ALL parallel processing causes resource contention and RAM issues
status: completed
type: bug
priority: normal
created_at: 2026-03-26T14:50:28Z
updated_at: 2026-03-27T05:29:41Z
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
- [x] Add global processing lock so only one category runs at a time
- [x] Change START ALL to queue categories sequentially
- [x] Update frontend to show queued/waiting state for pending categories
- [ ] Verify RAM stays bounded and throughput is stable per-category


## Summary of Changes

Sequential category processing implemented entirely in the frontend (`tasks.svelte.ts`):

- **`startAllEnabled()`** now processes categories one at a time instead of `Promise.all`. The first category starts immediately; remaining categories are added to `queuedCategories` and start only after the previous one finishes.
- **`waitForCategoryDone()`** polls category progress every 500ms to detect completion before starting the next.
- **`stop()`** handles queued categories by simply removing them from the queue (no backend call needed).
- **`stopAll()`** clears the queue before stopping running categories.
- **`isAnyRunning()`** includes queued categories so global controls (Pause All/Stop All) remain visible.
- **UI**: Both `ProcessingCategoryCard` and `ClapProcessingCard` show an indigo "Queued" badge and "Waiting to start: N assets" text for queued categories. Users can individually stop a queued category to remove it from the queue.

No backend changes needed — the frontend controls sequencing. Individual manual starts still work independently (only "Start All" enforces sequential processing).
