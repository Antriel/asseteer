---
# asseteer-fbpx
title: Eliminate cache thrashing via global ZIP-locality batch grouping
status: completed
type: task
priority: high
created_at: 2026-03-21T09:37:10Z
updated_at: 2026-03-21T11:44:46Z
parent: asseteer-k1go
---

## Problem

The current batch building in `work_queue.rs:build_work_batches()` groups consecutive assets by nested ZIP key, but only processes consecutive runs. Assets from the same nested ZIP that appear non-consecutively in the input list get split into separate batches. With 23 workers pulling from a shared channel, batches from different nested ZIPs interleave, causing the single-slot ZipCache to thrash.

### Evidence from logs

The same nested ZIP gets activated repeatedly throughout processing:
- `Doomed - Heavy Metal Music Collection.zip` activated **7 times** (lines 148, 162, 212, 239, 270, 280, 328)
- `Adventure Game Music Collection.zip` — **6 times**
- `Epic Boss Battles Music Collection 2.zip` — **6 times**

Each activation means waiting for the current key to drain, evicting cached data, then re-decompressing the nested ZIP.

ZipGate wait times escalate through the session as convoy builds:
- Early: 6-15s
- Mid: 40-80s
- Late: 98s, 104s, 115s, 122s, 134s

### Two sub-problems

1. **Batch size cap of 8** (`NESTED_ZIP_BATCH_SIZE = 8`): A nested ZIP with 10+ files gets split into multiple batches. Between those batches, other ZIPs steal the cache slot.

2. **Only consecutive grouping**: If the asset list has `[zip_a_file1, zip_b_file1, zip_a_file2]`, zip_a gets two separate batches instead of one.

## CLAP processing note

CLAP has implicit locality from the SQL ORDER BY (`folder_id, rel_path, zip_file, zip_entry` in `process.rs:57-64`), so assets from the same nested ZIP are mostly consecutive in the input. However, `build_work_batches` for CLAP uses blind `chunks(CLAP_BATCH_SIZE)` which doesn't respect ZIP key boundaries — a chunk of 8 can span the boundary between two nested ZIPs, causing cache thrashing within that batch. The fix is to ensure CLAP chunk boundaries align with ZIP key boundaries.

## Files to modify

- `src-tauri/src/task_system/work_queue.rs` — `build_work_batches()` (lines 131-199), dispatch logic, constants
- `src-tauri/src/task_system/processor.rs` — `process_clap_embedding_batch()` (lines 313-420), byte loading order within a batch

## Current code reference

```rust
// work_queue.rs lines 17-18
const CLAP_BATCH_SIZE: usize = 8;
const NESTED_ZIP_BATCH_SIZE: usize = 8;

// CLAP batching — chunks without respecting ZIP key boundaries
ProcessingCategory::Clap => assets
    .chunks(CLAP_BATCH_SIZE)
    .map(|chunk| WorkBatch { ... })
    .collect(),

// Image/Audio batching — only consecutive grouping, capped at 8
for asset in assets {
    let asset_key = zip_cache::nested_zip_group_key(&asset);
    match asset_key {
        Some(key) if current_key == Some(&key) && current_assets.len() < NESTED_ZIP_BATCH_SIZE => {
            current_assets.push(asset); // same key, add to batch
        }
        Some(key) => {
            flush_current(...); // different key or batch full, start new batch
            current_key = Some(key);
            current_assets.push(asset);
        }
        None => { /* non-nested: singleton batch */ }
    }
}
```

## Design: Staged dispatch

### Why simple sorting isn't enough

Even with globally sorted batches in a FIFO channel, thread scheduling is non-deterministic. Worker 5 (holding key B) could enter the ZipGate before Worker 1 (holding key A), activating key B with only 1 worker while Workers 1-4 (all holding key A batches) block. This degrades to serial processing with wasted parallelism.

### Staged dispatch approach

Instead of pushing all batches into the channel upfront, use a **dispatcher** that controls what's available to workers:

```
Batch groups (pre-sorted):
  ZIP groups:     [A1,A2,A3,A4], [B1,B2,B3], [C1,C2], [D1,D2,D3,D4,D5]
  Non-ZIP pool:   [n1, n2, n3, n4, n5, n6, n7, ...]

Dispatcher loop:
  1. Push all batches for key A into channel → workers grab & process in parallel
  2. While waiting for A to complete, also push non-ZIP batches to fill idle workers
  3. All A batches done → push key B batches
  4. Continue filling with non-ZIP batches between key groups
  5. Repeat until all work done
```

This guarantees:
- All workers processing ZIP batches have the SAME key (no gate races)
- Full parallelism within each key (multiple workers read from shared Arc<Vec<u8>>)
- Non-ZIP batches keep remaining workers saturated during key transitions
- No changes needed to ZipGate or ZipCache

### For CLAP processing

CLAP has concurrency=1 (single worker), so dispatch ordering is simpler. Just sort assets by nested ZIP key before chunking into CLAP_BATCH_SIZE batches. Within each batch, `process_clap_embedding_batch()` loads bytes sequentially, so same-key grouping avoids cache thrashing within a batch.

### Interaction with multi-slot cache (asseteer-5nvp)

This staged dispatch is designed to be **upgraded** by the multi-slot cache:
- With single-slot cache: dispatch one key group at a time (this bean)
- With multi-slot cache: dispatcher queries cache budget, can dispatch multiple key groups simultaneously if memory allows

The dispatcher interface should be designed with this upgrade path in mind, even if the initial implementation only supports one active key group.

## Revised implementation plan

- [x] Group all batches by nested ZIP key globally (sort, not just consecutive)
- [x] Separate non-ZIP batches into a fill pool
- [x] Implement dispatcher that sends one key group at a time + non-ZIP fill batches
- [x] Track completion of key groups (counter per group, decrement on batch done)
- [x] When key group completes, dispatch next key group
- [x] Apply same sorting to CLAP batch building
- [x] Remove NESTED_ZIP_BATCH_SIZE cap (keep batches at reasonable size for parallelism, e.g. 8, but all from same key)


## Non-ZIP batch metering

The dispatcher must NOT push all non-ZIP batches at once — doing so would effectively serialize ZIP processing behind all non-ZIP work (workers would be saturated with non-ZIP work, and in single-slot mode there'd be nothing to interleave with the active ZIP key).

Instead, non-ZIP batches are used as **fill** to occupy idle workers:

```
23 workers total

1. Push A batches (4) → 4 workers busy with ZIP A, 19 idle
2. Push ~19 non-ZIP batches → 19 workers busy with non-ZIP, 0 idle
3. Workers finish and re-request work:
   - Non-ZIP worker finishes → dispatcher gives another non-ZIP batch (if available)
   - A-batch worker finishes → if more A batches exist, give those, otherwise non-ZIP
4. All A batches done → push B batches (3) + refill non-ZIP for remaining idle workers
```

The goal: at any point, idle workers get non-ZIP fill, while ZIP key groups proceed at full parallelism. ZIP and non-ZIP work progress concurrently throughout.

Implementation option: use two channels — one for ZIP batches (dispatched in key groups), one for non-ZIP (always available). Workers try the ZIP channel first, fall back to non-ZIP. Or: single channel but dispatcher monitors channel depth and tops up non-ZIP batches as workers drain them.

## Summary of Changes

Implemented staged ZIP-locality dispatch in `src-tauri/src/task_system/work_queue.rs`:

1. **Two-channel architecture**: Replaced single crossbeam channel with `zip_tx/rx` (high priority) and `nonzip_tx/rx` (low priority). Workers check ZIP channel first, falling back to non-ZIP.

2. **Global ZIP key grouping**: `build_batch_plan()` replaces `build_work_batches()`, using a HashMap to group ALL assets by nested ZIP key globally (not just consecutive runs).

3. **Staged dispatcher**: A spawned tokio task dispatches one ZIP key group at a time. Each group gets a `BatchGroupCompletion` tracker (AtomicUsize + Notify). Workers decrement after processing; when it hits 0, the dispatcher advances to the next group.

4. **CLAP key-boundary chunking**: CLAP batches are sorted by key and chunked with key-boundary awareness, preventing cache thrashing within a single batch.

5. **ZIP groups sorted by size**: Largest key groups dispatched first for better pipelining.

All 56 existing tests pass + 5 new tests added (global grouping, non-consecutive grouping, large group splitting, CLAP key boundaries, completion tracking).
