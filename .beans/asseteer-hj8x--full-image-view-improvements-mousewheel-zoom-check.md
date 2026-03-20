---
# asseteer-hj8x
title: 'Full image view improvements: mousewheel zoom, checkerboard transparency, pixel-art mode'
status: in-progress
type: feature
priority: normal
created_at: 2026-03-20T08:26:33Z
updated_at: 2026-03-20T09:46:45Z
parent: asseteer-kvnt
---

Improve the full image viewer (opened on click):
- Mouse wheel to zoom in/out
- Show transparent areas as a standard checkerboard grid (not solid color)
- At high zoom levels, switch to non-interpolated pixel-art rendering
Note: there is a working setup in another project to reference when implementing this.

## Implementation Progress
- [x] Mouse wheel zoom (toward cursor position, min 10% / max 4000%)
- [x] Click-and-drag panning with pointer capture
- [x] Checkerboard transparency background on the image
- [x] Pixel-art (nearest-neighbor) rendering at ≥4x zoom
- [x] Image bounds outline toggle (dashed blue outline, `B` key)
- [x] Fit-to-view (`0` key) and actual-size 1:1 (`1` key) buttons
- [x] Compact toolbar layout replacing bottom control bar
- [x] Bottom info bar showing path and file size
- [x] Metadata panel toggle (`I` key)
- [x] Preserved: arrow key/button navigation, open in explorer, show in folder
