---
# asseteer-r4c0
title: Copy file path from asset context menu / button
status: completed
type: feature
priority: normal
created_at: 2026-04-03T08:21:18Z
updated_at: 2026-09-25T11:29:42Z
---

Add a way to copy the full file path of an asset to the clipboard, accessible via a button and/or the asset context menu. Should work for both audio and image assets.

## Summary of Changes
- "Copy Path" item in the shared `AssetContextMenu` (audio list, image grid, image list) → `copyAssetPath(asset)` in `$lib/actions/assetActions.ts`.
- Regular files copy their real path with OS separators. ZIP entries copy their location inside the archive (`...\Retro Pack.zip\Sounds\retro_coin.wav`); no extraction just to copy text. For getting a zipped sample into another program, drag-out (asseteer-3e94) is the path.
- New `ClipboardIcon` (a two-squares "copy" glyph looked just like the Find Similar icon above it).
- Verified in the harness: menu item in both themes, clipboard receives the expected paths, success toast.
