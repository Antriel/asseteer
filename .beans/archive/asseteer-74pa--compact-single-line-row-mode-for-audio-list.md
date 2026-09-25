---
# asseteer-74pa
title: 'Leaner audio browser: single-line rows + docked transport'
status: completed
type: feature
priority: normal
created_at: 2026-09-25T08:09:29Z
updated_at: 2026-09-25T09:22:51Z
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
- [x] verify via harness (both themes)

## Summary of Changes
Not a mode: the audio view was redesigned so the lean layout is the only one.
- **Rows** (`AudioList.svelte`): 32px single line, hairline `border-subtle` dividers, no cards/icons. Columns: play/pause glyph (on hover and when selected) · filename · folder *relative to the source* (`getAssetRelativeDirectory`, truncates from the left) · ZIP · similarity · duration · rate · channels · format · size. Rate/channels/size drop out via container queries when narrow.
- **Selected row**: accent tint + 2px accent edge + a *playhead wash*, a faint fill that tracks playback progress inside the row.
- **Transport strip**: replaces the floating player card; docked at a fixed 68px, so there's no layout jump between idle and selected. Idle state shows the keyboard shortcuts. Metadata, volume and Test loop drop out via container queries when narrow.
- Fixed a pre-existing race in `AudioPlayer`: a zip entry's async blob could resolve after a later selection and replace its source (the strip showed one file while another played). Load token + spec `tests/e2e/audio.spec.mjs` (verified it fails without the fix).
- `PlayIcon`/`PauseIcon` non-circled variants were identical to circled; they're now real glyphs.
- svelte-check was picking up Edge's scripts from the harness WebView profile (`tests/harness/out/profile`); excluded via `kit.typescript.config`.
