---
# asseteer-bz7r
title: 'Nested ZIP processing: limit parallelism to prevent OOM; check available memory'
status: in-progress
type: bug
priority: critical
created_at: 2026-03-19T11:42:44Z
updated_at: 2026-03-19T11:56:33Z
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
