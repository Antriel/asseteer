---
# asseteer-bz7r
title: 'Nested ZIP processing: limit parallelism to prevent OOM; check available memory'
status: completed
type: bug
priority: critical
created_at: 2026-03-19T11:42:44Z
updated_at: 2026-03-19T16:22:48Z
parent: asseteer-kvnt
---

PC froze with OOM error while processing a pack containing a nested 1 GB ZIP. Workers are likely opening the same large nested ZIP in parallel many times (observed ~30 GB RAM usage). asseteer-mxsj's 'one at a time' limit was for CLAP only. Need to: (1) generalize the single-worker-per-nested-ZIP constraint to normal file processing and playback/loading too, (2) add an upfront memory availability check before queuing nested ZIP work, (3) review the worker code to ensure no more than one worker decompresses any given nested ZIP concurrently.


## Implementation

### Shared nested ZIP cache (`zip_cache.rs`)
- Global `Mutex<Option<CachedInnerZip>>` holds at most one decompressed inner ZIP
- Mutex serializes all nested ZIP access — only one thread decompresses at a time
- Non-ZIP and simple ZIP assets bypass the cache entirely (no contention)
- `clear()` frees memory when processing completes or stops

### Sort by ZIP path for all categories
- Image and Audio queries now use `ORDER BY folder_id, rel_path, zip_file, zip_entry` (CLAP already had this)
- Consecutive files from the same nested ZIP maximize cache hits

### Unified code path
- `process_image()`, `process_audio()`, `generate_thumbnail_for_asset()`, and CLAP batch processing all use `zip_cache::load_asset_bytes_cached()`
- Removed the CLAP-specific `load_asset_bytes_cached` from `processor.rs`
- Cache cleared on processing completion and stop

### What this prevents
- Multiple workers decompressing the same 1GB+ inner ZIP in parallel (was causing ~30GB RAM / OOM)
- Memory bounded to one decompressed inner ZIP at a time

## Follow-up review work (2026-03-19)

- [x] Refactor nested ZIP cache to share immutable inner ZIP bytes instead of locking a mutable archive per read
- [x] Restore support for deeper-than-one-level nested ZIP paths
- [x] Make processing queue consume adjacent nested-ZIP assets in batches to reduce cache churn during metadata extraction

- [x] Validate metadata extraction throughput and stability on a real large multi-GB nested-ZIP pack

## Debug follow-up (2026-03-19)

- [x] Add timing logs for nested-ZIP cache wait/fill and audio metadata probe phases
- [x] Log nested-ZIP batch enqueue/dequeue behavior to confirm locality scheduling on real data
- [x] Relax nested-ZIP audio timeout handling so slow cache fills do not masquerade as deadlocks

## Coordinator follow-up (2026-03-19)

- [x] Add a global nested-ZIP active-key coordinator so different inner ZIPs do not thrash the one-slot cache

## Summary of Changes

- Replaced the original single locked nested-ZIP archive approach with shared immutable cached inner-ZIP bytes and per-read archive opens, preventing duplicate decompression without serializing all reads.
- Restored deeper-than-one-level nested ZIP support and fixed cache-key selection to target the deepest nested ZIP layer actually being reused.
- Added locality-aware batching plus a global active-key coordinator so one nested ZIP key is processed at a time while multiple workers can still read from that key in parallel.
- Investigated the slow metadata-processing path with targeted diagnostics, confirmed the main bottleneck was cross-key cache thrash rather than audio probing, and validated on a real large nested-ZIP pack that throughput improved substantially after the coordinator fix.

## Follow-up Beans

- `asseteer-8njz`: clean up temporary nested-ZIP debug instrumentation and restore production timeout behavior
- `asseteer-klss`: improve initial nested-ZIP scan/import throughput
- `asseteer-8p8a`: add an upfront memory-availability guard before large nested-ZIP processing
