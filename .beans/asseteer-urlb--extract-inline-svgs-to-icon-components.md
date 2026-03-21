---
# asseteer-urlb
title: Extract inline SVGs to icon components
status: completed
type: task
priority: low
created_at: 2026-03-20T11:43:57Z
updated_at: 2026-03-21T08:17:29Z
parent: asseteer-38rb
---

The project has an icon system at `src/lib/components/icons/` but several components use inline SVGs instead:

- **External link icon** — used in AudioList.svelte (line 280), ImageGrid.svelte (line 182), AssetList.svelte (line 148)
- **Copy/similarity icon** — used in AudioList.svelte (lines 264, 409) and Toolbar.svelte (line 364)
- **Brain/AI icon** — used in Toolbar.svelte (line 309)
- **Volume icon** — used in AudioPlayer.svelte (line 348)
- **Settings/gear icon** — used in Toolbar.svelte (line 320)
- **Search icon (large)** — used in library/+page.svelte (line 151)
- **Empty inbox icon** — used in library/+page.svelte (line 182)

These inline SVGs are harder to update consistently and add visual noise to templates.

**Suggested approach:**
Add ExternalLinkIcon, SimilarIcon, BrainIcon, VolumeIcon, GearIcon, InboxIcon to the icon system and replace inline SVGs.

## Summary of Changes

Created 6 new icon components in `src/lib/components/icons/`:
- `ExternalLinkIcon.svelte` — arrow-out-of-box external link
- `SimilarIcon.svelte` — copy/clipboard icon used for "Find Similar"
- `BrainIcon.svelte` — lightbulb/brain icon for semantic search
- `VolumeIcon.svelte` — speaker/volume icon
- `GearIcon.svelte` — settings cog icon
- `InboxIcon.svelte` — inbox/tray icon (supports `xl` size for empty states)

Also added `xl` size variant to `SearchIcon.svelte`.

All new icons exported from `index.ts` and inline SVGs replaced in:
- `AudioList.svelte` (3 SVGs → SimilarIcon × 2, ExternalLinkIcon × 1)
- `AudioPlayer.svelte` (1 SVG → VolumeIcon)
- `shared/Toolbar.svelte` (3 SVGs → BrainIcon, GearIcon, SimilarIcon)
- `routes/(app)/library/+page.svelte` (2 SVGs → SearchIcon xl, InboxIcon xl)

`ImageGrid.svelte` and `AssetList.svelte` had no inline SVGs remaining (already cleaned up).
