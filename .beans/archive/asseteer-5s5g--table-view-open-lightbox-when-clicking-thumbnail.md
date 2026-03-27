---
# asseteer-5s5g
title: 'Table view: open lightbox when clicking thumbnail'
status: completed
type: feature
priority: normal
created_at: 2026-03-16T11:42:25Z
updated_at: 2026-03-16T15:16:15Z
---

Clicking a thumbnail in the table view should open the lightbox/preview, same as in the grid view.

## Summary of Changes

Added lightbox opening on thumbnail click in AssetList.svelte (table view):
- Imported `viewState` from `$lib/state/view.svelte`
- Wrapped the `AssetThumbnail` in a `<button>` with `onclick` that calls `viewState.openLightbox(asset)`
- The lightbox modal is already rendered in the library page, so no other changes needed
