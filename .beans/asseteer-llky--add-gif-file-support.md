---
# asseteer-llky
title: Add GIF file support
status: completed
type: feature
priority: normal
created_at: 2026-03-16T11:42:37Z
updated_at: 2026-03-16T15:23:32Z
---

Support GIF files in the asset library — scanning, thumbnail generation, and display.

## Tasks
- [x] Enable `gif` feature in image crate (Cargo.toml)
- [x] Add GIF badge overlay on thumbnails in grid/list views
- [x] Verify lightbox displays animated GIFs correctly (uses original file, browser handles animation natively)

## Summary of Changes

- Enabled `gif` feature in `image` crate (`Cargo.toml`) so GIF files are decoded during thumbnail generation
- Added semi-transparent "GIF" badge overlay (top-left corner) on thumbnails in the image grid view
- Added "GIF" info badge next to filename in the table/list view
- Lightbox already serves original files, so animated GIFs play natively in the browser
