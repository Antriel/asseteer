---
# asseteer-j3v0
title: Fix pending counts and processing queries for thumbnail pre-generation setting
status: completed
type: bug
priority: normal
created_at: 2026-03-20T11:30:09Z
updated_at: 2026-03-20T11:33:28Z
---

When user processes assets with pre_generate_thumbnails=false, images get metadata but NULL thumbnails. If they later toggle the setting on, getPendingAssetCounts only counts images without metadata (im.asset_id IS NULL), so count shows 0 and processing can't be triggered. Backend start_processing also only queries for missing metadata, not missing thumbnails.


## Summary of Changes

- `src/lib/database/queries.ts`: `getPendingAssetCounts` now accepts `preGenerateThumbnails` param; when true, also counts images that have metadata but `thumbnail_data IS NULL`
- `src/lib/state/tasks.svelte.ts`: passes `settings.preGenerateThumbnails` to `getPendingAssetCounts` so counts reflect current setting
- `src-tauri/src/commands/process.rs`: Image query in `start_processing` now also fetches assets with existing metadata but null thumbnail when `pre_generate_thumbnails` is true; processor already handles upsert correctly
