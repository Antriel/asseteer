---
# asseteer-2n9u
title: Lazy-load thumbnails on demand
status: completed
type: feature
priority: normal
created_at: 2026-03-16T11:42:37Z
updated_at: 2026-03-16T15:35:54Z
---

Instead of processing all thumbnails upfront, generate them lazily when assets become visible. Avoids forcing full processing before thumbnails appear. Also fixes the issue where frontend doesn't know if a thumbnail was skipped (small image) or just not yet generated.

## Plan

- [x] Backend: new `ensure_thumbnails` Tauri command (batch of asset IDs → generate missing thumbnails, store in DB)
- [x] Backend: remove thumbnail generation from `process_image` (keep dimension extraction only)
- [x] Backend: add ON CONFLICT handling so processing and lazy generation don't collide
- [x] Frontend: new `thumbnailManager.svelte.ts` with batching + debounce
- [x] Frontend: update `ImageThumbnail.svelte` to use manager
- [x] Frontend: update `AssetThumbnail.svelte` to use manager

## Summary of Changes

- **Backend**: New `ensure_thumbnails` Tauri command generates missing thumbnails on demand for a batch of asset IDs, with upsert handling for race conditions
- **Backend**: `process_image` now only extracts dimensions (no thumbnail generation), with ON CONFLICT to preserve lazily-generated thumbnails
- **Backend**: New `generate_thumbnail_for_asset` public function extracted from processor for reuse
- **Frontend**: New `thumbnails.svelte.ts` state module with batched/debounced thumbnail requests, SvelteMap cache, and blob URL lifecycle management
- **Frontend**: `ImageThumbnail.svelte` and `AssetThumbnail.svelte` rewritten to use the centralized thumbnail manager
- **Frontend**: Thumbnail cache cleared automatically when search/asset list changes
