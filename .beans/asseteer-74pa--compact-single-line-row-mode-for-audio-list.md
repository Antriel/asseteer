---
# asseteer-74pa
title: Compact single-line row mode for audio list
status: todo
type: feature
priority: normal
created_at: 2026-09-25T08:09:29Z
updated_at: 2026-09-25T08:09:29Z
---

User request: "make the interface a bit leaner/single line for each sample, makes browsing a lot faster and cleaner".

## Current
`src/lib/components/AudioList.svelte`: `itemHeight = 88`, each row is a `h-20 p-4` card with a 48px icon, filename + path on line 1, metadata (duration, format, size…) on line 2, similarity badge. Rendered via `VirtualList`.
(`AssetList.svelte` uses `rowHeight = 81` for images-as-list — consider same treatment.)

## Proposal
- Density setting (comfortable / compact), persisted in `settings.svelte.ts`, toggle in the toolbar near `ViewModeToggle`.
- Compact: ~28–32px rows, no card border/gap (zebra or hairline divider instead), small play icon, one line: filename · path (truncating) · duration · format · size right-aligned in fixed-width columns.
- Keep selection, keyboard nav, context menu, similarity badge working.
- VirtualList itemHeight must follow the mode.
- Load `interface-design` skill for the design pass.

## Verify
Harness screenshots of both modes, light + dark, with the fixture library; check keyboard nav scroll-into-view still aligns with new row height.

- [ ] density setting + toggle
- [ ] compact row markup
- [ ] VirtualList height per mode
- [ ] decide on AssetList (images list) too
- [ ] verify via harness (both themes)
