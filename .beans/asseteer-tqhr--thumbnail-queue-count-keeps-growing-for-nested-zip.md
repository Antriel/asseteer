---
# asseteer-tqhr
title: Thumbnail queue count keeps growing for nested-ZIP images without scrolling
status: in-progress
type: bug
priority: normal
created_at: 2026-03-21T12:07:25Z
updated_at: 2026-03-23T09:19:42Z
---


After importing a mostly zip-based bundle and opening the images view, each zip contains one large image file (nested zips). Thumbnails show as loading (expected, since unpacking takes time). However, the queued thumbnail count in the status bar keeps increasing even without any scrolling. Also suspect the virtual list padding rows (how far beyond visible area we queue thumbnails) may be larger than the intended 2 rows — worth double-checking.


## Investigation

The "Thumbnails queued" label was misleading. When thumbnails are pre-generated during processing, they exist in DB but aren't in the frontend cache yet. As the user scrolls, images trigger `requestThumbnail()` → backend worker → checks DB → already exists → emits `thumbnail-ready`. This is loading, not queuing.

### Fix applied
- Changed label from "Thumbnails: X queued" to "Loading thumbnails: X"
- Also fixed the idle status bar progress bar color (`bg-secondary` was nearly invisible against `bg-tertiary` track) — changed to `bg-accent/40`
- Removed inner `rounded-full` from progress bar child div to prevent sub-pixel rendering artifacts at 100%


## Root Cause (Deeper Investigation)

The growing count is caused by `clearThumbnailCache()` (called on folder navigation / asset reload via `loadAssets()`):

1. `requested.clear()` — frontend forgets which IDs it requested
2. `cancelBuffer = []` — pending cancels are discarded, never sent to backend
3. Component unmounts call `cancelThumbnail(id)` → `requested.has(id)` is false (just cleared) → no cancel sent
4. Backend worker still has old IDs in `pending` + `in_flight`
5. New components mount → `requestThumbnail` passes dedup (everything cleared)
6. Backend: completed IDs (removed from `in_flight`) get re-added to `pending`
7. `pending` grows: old uncancelled items + re-added completed duplicates

Additionally, `thumbnail-ready` handler deleted from `requested` before `cache.set` (async DB read gap). The `assets.svelte.ts` handler patching `asset.width`/`height` could trigger effect re-runs that hit this dedup window.

## Fixes Applied
- Added `ClearAll` message to `ThumbnailMsg` enum
- Added `clear_all()` to `WorkerState` — resets pending, in_flight, cancelled, stats
- Added `clear_thumbnail_queue` Tauri command
- `clearThumbnailCache()` now calls `invoke('clear_thumbnail_queue')` to sync frontend+backend
- Moved `requested.delete(asset_id)` to AFTER `cache.set`/`failed.add` in `thumbnail-ready` handler
