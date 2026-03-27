---
# asseteer-euls
title: 'Image/thumbnail view: right-click context menu with ''Open in Explorer'' and ''Show in Folder'''
status: completed
type: feature
priority: normal
created_at: 2026-03-20T08:26:17Z
updated_at: 2026-03-20T09:40:43Z
parent: asseteer-kvnt
---

Add right-click context menu to image thumbnails and the full image view. Should include: 'Open in Explorer' (opens file in system file explorer), and 'Show in Folder' (navigates to the asset's folder in our library folders view). Audio files already have this — match that pattern.

## Summary of Changes

- `ImageGrid.svelte`: Added right-click context menu on thumbnails with "Show in Folder" and "Open in File Explorer" options, matching the AudioList pattern.
- `ImageLightbox.svelte`: Added "Open in File Explorer" button in the controls bar next to the existing "Show in Folder" button.
