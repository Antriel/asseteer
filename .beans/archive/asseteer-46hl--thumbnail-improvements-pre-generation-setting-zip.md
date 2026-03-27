---
# asseteer-46hl
title: 'Thumbnail improvements: pre-generation setting, ZIP compression tracking, smarter skip logic'
status: completed
type: feature
priority: normal
created_at: 2026-03-20T10:53:10Z
updated_at: 2026-03-20T11:01:43Z
---

Improve thumbnail generation with three related changes:

## Changes

### 1. `zip_compression` column on `assets`
Add `zip_compression TEXT` (values: `store`, `deflate`, `zstd`, etc.) to the `assets` table. Populate during ZIP scanning — value is in the central directory, no decompression needed. Clean break, no migration.

### 2. Pre-generation setting (Option A)
Add a global setting "Pre-generate thumbnails during scan". When enabled, the processor generates and stores thumbnails inline during the main scan (not a separate pass). The lazy worker remains as fallback for anything missed.

The `ON CONFLICT` clause already protects existing thumbnails from being overwritten on re-scan.

### 3. Smarter skip logic
Update both the pre-generation path and the lazy worker's `find_missing_thumbnails` query:
- Skip if image ≤ 128px (existing rule — keep)
- Skip if ZIP STORE mode AND not nested ZIP (new rule)
- Never skip for nested ZIPs (new explicit rule)

## Todo
- [ ] Add `zip_compression` to assets schema (clean break)
- [ ] Populate `zip_compression` during ZIP scanning in processor/scanner
- [ ] Add "pre-generate thumbnails" global setting to settings store
- [ ] Add setting UI (global settings panel)
- [ ] Modify processor to generate thumbnail inline when setting is enabled
- [ ] Update skip logic in lazy worker (`find_missing_thumbnails`)
- [ ] Update skip logic in pre-generation path


## Summary of Changes

- **`schema.rs`**: Added `zip_compression TEXT` column to `assets` table
- **`models.rs`**: Added `zip_compression: Option<String>` to `Asset` struct
- **`scan.rs`**: Added `zip_compression` to `DiscoveredAsset`, captured via `entry.compression()` in the ZIP scanner first pass, stored in `insert_asset_chunk`. Added `compression_method_str()` helper (store/deflate/deflate64/bzip2/zstd/lzma)
- **`rescan.rs`**: Added `zip_compression` to INSERT for added assets and UPDATE for modified assets
- **`processor.rs`**: `process_asset` now takes `pre_generate_thumbnails: bool`. When true and image >128px, generates thumbnail inline (same blocking task). Uses CASE WHEN ON CONFLICT to never overwrite an existing thumbnail
- **`work_queue.rs`**: `WorkBatch` and `WorkQueue::start` now carry `pre_generate_thumbnails`, threaded down to `process_asset`
- **`process.rs`**: `start_processing` command accepts `preGenerateThumbnails: bool`; retry defaults to `false`
- **`thumbnail_worker.rs`**: `find_missing_thumbnails` now skips STORE-mode non-nested ZIP entries (`zip_compression = 'store' AND zip_entry NOT LIKE '%.zip/%'`)
- **`settings.svelte.ts`** (new): localStorage-persisted settings state with `preGenerateThumbnails`
- **`settings/+page.svelte`**: Added "Processing" section with toggle for pre-generate thumbnails
- **`tasks.svelte.ts`**: `startProcessing` passes `preGenerateThumbnails` from settings to `start_processing`
