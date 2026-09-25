---
# asseteer-ugch
title: Multi-select in library lists (and multi-file drag-out)
status: completed
type: feature
priority: normal
created_at: 2026-09-25T11:29:37Z
updated_at: 2026-09-25T11:45:29Z
---

Lists currently track a single selectedAsset. Add Ctrl/Shift multi-select to AudioList/ImageGrid; dragging a selected row then drags all selected files (start_asset_drag takes one id today → accept a list). Follow-up to asseteer-3e94.

## Implementation (awaiting manual test)
- `ListSelection` (`src/lib/state/listSelection.svelte.ts`, one per list component) and pure `rangeIds`/`actionTargets` in `src/lib/utils/selection.ts` (unit-tested).
- **AudioList**: plain click selects that row and plays it, as before. Ctrl+click toggles and Shift+click selects a range, neither plays. Shift+↑/↓ extends while auditioning; plain arrows collapse; Esc collapses to the current sound. Auto-advance ("play next") keeps a multi-selection. Selected rows get the accent wash, and the transport shows "N selected".
- **ImageGrid / AssetList**: Ctrl/Shift+click select (accent ring / row wash). A plain click opens the lightbox and clears the selection.
- Drag or Copy Path on a selected item applies to the whole selection in list order ("Copy N Paths", one path per line). On an unselected item it applies to just that item.
- Selection is pruned to rows still in the list when results change.
- `start_asset_drag` takes `assetIds` (chunked `IN` query, keeps the order it was sent). The drag preview reads "♪ first.wav +N more".
- Harness-verified: ctrl/shift/Esc/Shift+Down counts; drag ids [coin, powerup] in list order; unselected drag sends one id; two-line clipboard; image tiles ring and drag both ids; both themes; no console errors; e2e 15/15, unit 44, cargo 77.
- [x] Manual: drag 2+ selected sounds (incl. zipped) into Audacity / Explorer

## Summary of Changes
Multi-select (Ctrl/Shift+click, Shift+arrows, Esc) in the audio list, image grid, and image list. Drag and Copy Path apply to the whole selection. Peter confirmed multi-file drag works in the real app.
