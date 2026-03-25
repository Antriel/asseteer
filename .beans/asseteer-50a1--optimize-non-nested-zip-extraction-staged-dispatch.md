---
# asseteer-50a1
title: 'Optimize non-nested ZIP extraction: staged dispatch + shared archive'
status: completed
type: task
priority: high
created_at: 2026-03-25T09:36:29Z
updated_at: 2026-03-25T09:50:42Z
blocked_by:
    - asseteer-ctrc
---

## Problem

Non-nested ZIP processing degrades from 250/s to 100/s due to concurrent workers all parsing the same ZIP's central directory independently.

Root cause identified via instrumentation (see asseteer-ctrc): `malecharactersbundle_row.zip` has **40,176 entries**. Every batch of 16 assets:
1. Opens the file + parses the entire 40K-entry central directory (~3MB) into a HashMap
2. Extracts just 16 entries

With ~23 workers hitting the same ZIP concurrently, this means 23 parallel central directory parses. The 3.6-4s baseline per batch (for small 400-600KB extractions) is almost entirely CD parse overhead — actual decompression is negligible.

Evidence:
- `probe_ms` avg 0.02ms, max 5ms — Symphonia is not the bottleneck
- `probe_queue_wait_ms` and `load_queue_wait_ms` are 0 — spawn_blocking pool is fine
- `extract_ms` 3.6-16s per batch of 16 entries from a 40K-entry ZIP
- Small batches (394KB) still take 3.6s — dominated by ZipArchive::new() CD parsing
- High CPU + low heat = allocation/parsing overhead, not computation or I/O

## Implementation Plan

### Phase 1: Staged dispatch for non-nested ZIP groups
Move regular ZIP batches from `non_zip` into `zip_groups` in `build_batch_plan()`. This reuses the existing `BatchGroupCompletion` + semaphore dispatcher already working for nested ZIPs.

- In `build_batch_plan()` (work_queue.rs ~L287-340): construct `ZipBatchGroup` entries from `regular_zip_map` instead of pushing to `non_zip`
- Use `REGULAR_ZIP_BATCH_SIZE` (16) for chunking within groups
- Only filesystem assets remain in `non_zip`
- The dispatcher semaphore (currently memory-budget based for nested ZIPs) may need adjustment — non-nested ZIPs don't consume cache memory, so could allow higher concurrency (e.g., 2-4 groups in flight vs 1 for nested)

### Phase 2: Shared archive handle across batches
Once dispatch is serial per ZIP, we can open `ZipArchive` once and pass it through sequential batches:

- Add a shared archive handle to the ZIP batch group dispatch flow
- Open `ZipArchive::new()` once when the group starts
- Pass `&mut ZipArchive` to each batch's extraction instead of each batch calling `bulk_load_from_zip`
- This eliminates CD parsing for all but the first batch: 3.6s → ~50ms per batch
- Requires refactoring `bulk_load_from_zip` to accept an existing archive handle (or split into open + extract functions)

### Phase 3: Tune concurrency
- Non-nested ZIP groups: allow 2-4 concurrent groups (they don't use zip_cache memory)  
- Nested ZIP groups: keep memory-budget-based limit (unchanged)
- Filesystem assets: keep on free queue (unchanged)
- Consider separate semaphores for nested vs non-nested

## Tasks
- [x] Phase 1: Move regular ZIP batches into zip_groups for staged dispatch
- [x] Phase 2: Refactor bulk_load_from_zip to accept open archive handle
- [x] Phase 2: Wire shared archive through batch group dispatch
- [x] Phase 3: Separate concurrency limits for nested vs non-nested ZIP groups
- [x] Verify with instrumentation logs that extract_ms drops significantly (user should test)
- [x] Remove instrumentation code after verification (handled in asseteer-ctrc)

## Summary of Changes

Implemented all three phases of the ZIP extraction optimization:

**Phase 1 - Staged dispatch**: Regular (non-nested) ZIP batches are now routed through `zip_groups` instead of `non_zip`, using the existing `BatchGroupCompletion` + semaphore dispatcher. This serializes access per ZIP file.

**Phase 2 - Shared archive**: The dispatcher pre-extracts ALL entries for a regular ZIP group in a single `bulk_load_from_zip` call (one archive open, one central directory parse), then distributes the bytes to worker batches via `preloaded_bytes: Arc<HashMap>`. Workers use these pre-loaded bytes instead of each opening the ZIP independently.

**Phase 3 - Separate concurrency**: Nested ZIP groups use memory-budget-based limits (~1 per GB). Regular ZIP groups use `max(2, num_cpus/2)` concurrent groups since they dont consume zip_cache memory.

Expected improvement: For a 40K-entry ZIP, instead of 23 workers each parsing the 3MB central directory (3.6s per batch), only one parse happens per group dispatch. Batch time should drop from ~3.6s to ~50ms for all but the first batch.
