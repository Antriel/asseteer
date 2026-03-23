---
# asseteer-tqhr
title: Thumbnail queue count keeps growing for nested-ZIP images without scrolling
status: completed
type: bug
priority: normal
created_at: 2026-03-21T12:07:25Z
updated_at: 2026-03-23T11:17:12Z
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


## Debug & Root Cause (March 2026)

Added extensive logging to diagnose the growing count. Captured logs showed **1980 total requests from only 64 unique asset IDs** — the same IDs being recycled ~30 times each.

### Root cause: SvelteMap coarse-grained reactivity cascade

Svelte 5's `SvelteMap` is **coarse-grained**: any `cache.set()` call (regardless of key) invalidates ALL reactive consumers that ever called `cache.has()` or `cache.get()` on that map.

The `$effect` in `ImageThumbnail.svelte` called `requestThumbnail(asset.id)`, which internally calls `cache.has(assetId)`. This created a reactive dependency on the `cache` SvelteMap for every component.

**The cascade:**
1. Thumbnail X completes → `cache.set(X, url)` fires
2. SvelteMap notifies ALL 64 components' effects (coarse, not per-key)
3. Each effect runs its cleanup: `cancelThumbnail(assetId)` → removes from `requested`
4. Each effect re-runs its body: `requestThumbnail(assetId)` → re-adds to queue (since `requested` was just cleared)
5. Backend receives 63 new requests; those whose `in_flight` slot is free (already completed) get added to `pending`
6. Repeat for every thumbnail completion → pending grows to 1100+

### Fix applied

Wrapped `requestThumbnail(asset.id)` in `untrack()` in `ImageThumbnail.svelte`. This prevents `cache.has()` inside `requestThumbnail` from creating a reactive SvelteMap dependency, so the effect only runs on mount, `isSmallImage` change, or `cacheReset.version` change (explicit cache clear). The cancel+re-request cascade no longer fires on every thumbnail completion.
