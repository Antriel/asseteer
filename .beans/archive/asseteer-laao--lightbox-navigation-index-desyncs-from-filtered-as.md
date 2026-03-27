---
# asseteer-laao
title: Lightbox navigation index desyncs from filtered asset list
status: completed
type: bug
priority: normal
created_at: 2026-03-16T09:18:56Z
updated_at: 2026-03-16T14:54:15Z
parent: asseteer-cfrp
---

In view.svelte.ts:39-51, nextImage/prevImage use this.lightboxIndex to index into the assets array passed as a parameter. But lightboxIndex is set from ImageGrid's startIndex + idx (line 106 of ImageGrid.svelte), which is the index into the full assets array. If the assets array changes between when the lightbox was opened and when next/prev is called (e.g., a search result updates, processing completes, or tab switch), the index may point to the wrong asset or be out of bounds. This is a race condition — clicking next/prev fast during a live search could show wrong images.

## Summary of Changes

Replaced index-based lightbox navigation with asset ID-based lookup. Instead of storing a `lightboxIndex` that could desync when the filtered asset list changes, `nextImage`/`prevImage` now use `findIndex` on the current asset's ID to locate it in the (potentially updated) assets array. This eliminates the race condition where search/filter changes could cause the index to point to the wrong asset or go out of bounds.

Changed files:
- `src/lib/state/view.svelte.ts` — Removed `lightboxIndex` state, changed `openLightbox` to accept only the asset (no index), rewrote `nextImage`/`prevImage` to find current position by ID
- `src/lib/components/ImageGrid.svelte` — Removed index computation and passing from click handler
