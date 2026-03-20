---
# asseteer-xs88
title: 'Library image view: thumbnails stall after clicking sidebar folder filter'
status: completed
type: bug
priority: normal
created_at: 2026-03-19T11:40:43Z
updated_at: 2026-03-20T08:12:49Z
parent: asseteer-kvnt
---

In the Library images view, clicking a FOLDERS entry in the sidebar causes a view refresh. Already-loaded thumbnails start re-loading but never finish. Scrolling to new items works fine. No console errors. Likely a viewport-detection issue — the intersection observer or visibility tracking may not correctly re-trigger for items that were already in view before the filter changed.

## Root Cause

`onMount` in `ImageThumbnail.svelte` only runs once per component lifecycle. When a folder filter is clicked, `clearThumbnailCache()` wipes the `cache` SvelteMap, `failed`, and `requested` sets. Components already in the viewport have their `thumbnailUrl` derived value go null (cache cleared), but since `onMount` doesn't re-run, `requestThumbnail()` is never called again — permanent spinner.

Virtual scrolling works fine because destroying/recreating components (scroll out then back in) triggers fresh `onMount` calls.

## Fix

- Added `cacheReset = $state({ version: 0 })` to `thumbnails.svelte.ts`, incremented in `clearThumbnailCache()`
- Replaced `onMount` with `$effect` in `ImageThumbnail.svelte`
- The `else` branch reads `cacheReset.version` (via `void cacheReset.version`) so Svelte tracks it as a dependency — when the cache is cleared, the effect re-runs and calls `requestThumbnail(asset.id)` again

## Summary of Changes

- `src/lib/state/thumbnails.svelte.ts`: export `cacheReset` state, increment on `clearThumbnailCache()`
- `src/lib/components/ImageThumbnail.svelte`: `onMount` → `$effect`, import and track `cacheReset.version`
