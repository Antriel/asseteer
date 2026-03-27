---
# asseteer-qxu6
title: ImageThumbnail blob URLs leak when component is re-keyed
status: completed
type: bug
priority: normal
created_at: 2026-03-16T09:19:06Z
updated_at: 2026-03-16T14:50:21Z
parent: asseteer-cfrp
---

ImageThumbnail.svelte creates blob URLs via URL.createObjectURL (lines 40, 45) and only revokes them in the onMount cleanup (line 77-79). However, if the component is destroyed and recreated with the same assetId (e.g., virtual scrolling re-keys the item), the old blob URL from the previous instance leaks because cleanup only runs on unmount. With 5000 items and scrolling back and forth, this can accumulate hundreds of leaked blob URLs consuming memory. Additionally, there is no fallback loading for the get_asset_bytes invoke at line 43 — a large original image is loaded fully into memory for display when no thumbnail exists.

## Summary of Changes

Fixed blob URL memory leak in `ImageThumbnail.svelte` with two changes:

1. **`$effect` for blob URL lifecycle**: Moved `URL.revokeObjectURL` into a `$effect` cleanup that tracks `thumbnailUrl`. This ensures blob URLs are revoked both when the URL changes and when the component is destroyed, regardless of timing.

2. **`destroyed` flag for async cancellation**: Added early-return checks after each `await` in `loadThumbnail()`. If the component unmounts while the async fetch is in-flight, the blob URL is never created (preventing the leak where async completion races past the old cleanup).
