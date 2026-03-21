---
# asseteer-8x9h
title: Extract shared asset byte loading utility
status: completed
type: task
priority: low
created_at: 2026-03-20T11:45:02Z
updated_at: 2026-03-21T08:21:30Z
parent: asseteer-38rb
---

The pattern of loading ZIP asset bytes via invoke('get_asset_bytes') → Uint8Array → Blob → URL.createObjectURL is duplicated across 4 components:

- `src/lib/components/AssetThumbnail.svelte` (lines 52-56)
- `src/lib/components/ImageThumbnail.svelte` (lines 62-70)
- `src/lib/components/AudioPlayer.svelte` (lines 114-125)
- `src/lib/components/modals/ImageLightbox.svelte` (lines 150-158)

Each also needs corresponding URL.revokeObjectURL cleanup on unmount/change.

**Suggested approach:**
Create a utility function like `loadAssetBlobUrl(assetId: number, mimeType: string): Promise<string>` that handles the invoke + Blob + createObjectURL chain. Pair with a cleanup helper or return a disposable.


## CLAUDE.md Updates
When implementing this, add a note to `src/lib/database/CLAUDE.md` BLOB Handling section about the shared asset byte loading utility, so new components use it instead of raw invoke+Blob patterns.

## Todo

- [x] Create `src/lib/utils/assetBlob.ts` with `loadAssetBlobUrl`
- [x] Update `AssetThumbnail.svelte`
- [x] Update `ImageThumbnail.svelte`
- [x] Update `AudioPlayer.svelte`
- [x] Update `ImageLightbox.svelte`
- [x] Update `src/lib/database/CLAUDE.md`

## Summary of Changes

Created `src/lib/utils/assetBlob.ts` with `loadAssetBlobUrl(assetId, mimeType)`. Updated all 4 components to use it, removing their direct `invoke('get_asset_bytes')` calls. Also standardized `AssetThumbnail`, `ImageThumbnail`, and `ImageLightbox` from `number[]` to `ArrayBuffer` IPC (matching `AudioPlayer`'s existing approach) for reduced overhead. Updated `src/lib/database/CLAUDE.md` to document the utility.
