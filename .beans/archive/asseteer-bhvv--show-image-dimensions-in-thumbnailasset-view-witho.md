---
# asseteer-bhvv
title: Show image dimensions in thumbnail/asset view without reload
status: completed
type: bug
priority: normal
created_at: 2026-03-20T08:26:28Z
updated_at: 2026-03-20T09:28:26Z
parent: asseteer-kvnt
---

When thumbnails lazy-load, they trigger processing which produces width/height data. However this data isn't shown in the UI unless the image is unloaded and reloaded. Investigate whether the dimensions are available at thumbnail load time and surface them in the UI without requiring a reload. Should be an easy fix if the data is already in the DB after thumbnail generation.


## Summary of Changes

Added a `thumbnail-ready` event listener in `assets.svelte.ts` (module-level, after `assetsState` is created). When a thumbnail is successfully generated, the backend has already written the dimensions to `image_metadata`. The listener:
1. Finds the asset in the current list by ID
2. Skips if dimensions are already populated
3. Queries `image_metadata` for `width`/`height`
4. Patches them directly on the reactive `assets` array item

No Rust changes required. Svelte 5's deep `$state` proxy picks up the mutation immediately, so dimensions appear in the grid/list overlays without any reload.
