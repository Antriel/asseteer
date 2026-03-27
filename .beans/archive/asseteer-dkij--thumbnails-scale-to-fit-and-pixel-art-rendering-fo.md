---
# asseteer-dkij
title: 'Thumbnails: scale-to-fit and pixel-art rendering for small images'
status: completed
type: feature
priority: normal
created_at: 2026-03-20T08:26:31Z
updated_at: 2026-03-20T10:33:43Z
parent: asseteer-kvnt
---

Two issues with thumbnail rendering:
1. Currently appears to scale-to-fill; should scale-to-fit (letterbox/pillarbox) so the full image is visible without cropping.
2. For small images (e.g. <= 128x128), disable interpolation and render as pixel art (image-rendering: pixelated) instead of blurry upscaling.

## Summary of Changes

- `AssetThumbnail.svelte`: `object-cover` → `object-contain`, added `image-rendering: pixelated` for small images
- `ImageThumbnail.svelte`: same changes
