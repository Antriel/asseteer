---
# asseteer-dkij
title: 'Thumbnails: scale-to-fit and pixel-art rendering for small images'
status: todo
type: feature
created_at: 2026-03-20T08:26:31Z
updated_at: 2026-03-20T08:26:31Z
parent: asseteer-kvnt
---

Two issues with thumbnail rendering:
1. Currently appears to scale-to-fill; should scale-to-fit (letterbox/pillarbox) so the full image is visible without cropping.
2. For small images (e.g. <= 128x128), disable interpolation and render as pixel art (image-rendering: pixelated) instead of blurry upscaling.
