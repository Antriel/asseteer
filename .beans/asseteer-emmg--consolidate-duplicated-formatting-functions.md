---
# asseteer-emmg
title: Consolidate duplicated formatting functions
status: completed
type: task
priority: low
created_at: 2026-03-20T11:43:48Z
updated_at: 2026-03-21T08:12:22Z
parent: asseteer-38rb
---

Several formatting functions are duplicated across files:

1. **Duration formatting**: `formatDuration(ms)` in AudioList.svelte (line 41) and `formatTime(seconds)` in AudioPlayer.svelte (line 228) — same logic, different input units
2. **File size formatting**: `formatFileSize(bytes)` in AudioList.svelte (line 54) and `formatFileSize(bytes)` in assets.svelte.ts (line 196) — slightly different implementations (one uses toFixed(2), the other toFixed(1))
3. **Similarity formatting**: `formatSimilarity()` in AudioList.svelte (line 25) and in clap.svelte.ts (line 428) — identical implementations

**Suggested approach:**
Create `$lib/utils/format.ts` with canonical implementations: `formatDuration(ms)`, `formatFileSize(bytes)`, `formatSimilarity(score)`. Import everywhere instead of local definitions.


## CLAUDE.md Updates
When implementing this, add `$lib/utils/format.ts` to the root `CLAUDE.md` Key Patterns section so future work uses the canonical formatters instead of creating new ones.

## Summary of Changes

Created `$lib/utils/format.ts` with canonical implementations of `formatDuration(ms)`, `formatFileSize(bytes)`, and `formatSimilarity(score)`.

- **AudioList.svelte**: removed 3 local functions, now imports from utils
- **AudioPlayer.svelte**: removed `formatTime(seconds)`, now uses `formatDuration(seconds * 1000)`
- **assets.svelte.ts**: removed exported `formatFileSize`
- **clap.svelte.ts**: removed exported `formatSimilarity`
- **AssetList.svelte**: updated import from `assets.svelte` → `$lib/utils/format`
- **CLAUDE.md**: added formatting utilities entry to Key Patterns section
