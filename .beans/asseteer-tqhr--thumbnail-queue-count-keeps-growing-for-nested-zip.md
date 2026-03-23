---
# asseteer-tqhr
title: Thumbnail queue count keeps growing for nested-ZIP images without scrolling
status: in-progress
type: bug
priority: normal
created_at: 2026-03-21T12:07:25Z
updated_at: 2026-03-23T08:49:55Z
---


After importing a mostly zip-based bundle and opening the images view, each zip contains one large image file (nested zips). Thumbnails show as loading (expected, since unpacking takes time). However, the queued thumbnail count in the status bar keeps increasing even without any scrolling. Also suspect the virtual list padding rows (how far beyond visible area we queue thumbnails) may be larger than the intended 2 rows — worth double-checking.


## Investigation

The "Thumbnails queued" label was misleading. When thumbnails are pre-generated during processing, they exist in DB but aren't in the frontend cache yet. As the user scrolls, images trigger `requestThumbnail()` → backend worker → checks DB → already exists → emits `thumbnail-ready`. This is loading, not queuing.

### Fix applied
- Changed label from "Thumbnails: X queued" to "Loading thumbnails: X"
- Also fixed the idle status bar progress bar color (`bg-secondary` was nearly invisible against `bg-tertiary` track) — changed to `bg-accent/40`
- Removed inner `rounded-full` from progress bar child div to prevent sub-pixel rendering artifacts at 100%
