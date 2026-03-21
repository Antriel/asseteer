---
# asseteer-38rb
title: Frontend code cleanup for initial release
status: completed
type: epic
priority: normal
created_at: 2026-03-20T11:43:13Z
updated_at: 2026-03-21T08:26:24Z
---

Code quality improvements to reduce duplication and improve maintainability before initial release. Focus on consolidating duplicated patterns that will cause maintenance problems.


## Child Issues

### High Priority
- **asseteer-mhrg**: Extract shared asset context menu and actions — showInFolder/openDirectory/context menu copy-pasted across 3 components

### Normal Priority
- **asseteer-kxp3**: Remove obsolete processing state from ui.svelte.ts — unused isProcessing/processProgress superseded by tasks.svelte.ts
- **asseteer-xfu1**: Deduplicate query filter building in queries.ts — searchAssets and countSearchResults share duplicated filter logic
- **asseteer-lrzz**: Unify SemanticSearchResult with Asset type — redundant type definition causes awkward mapping

### Low Priority
- **asseteer-emmg**: Consolidate duplicated formatting functions — formatDuration, formatFileSize, formatSimilarity in multiple files
- **asseteer-8x9h**: Extract shared asset byte loading utility — invoke→Blob→URL pattern duplicated across 4 components
- **asseteer-urlb**: Extract inline SVGs to icon components — ~7 inline SVGs bypass the existing icon system
- **asseteer-qdes**: Clean up verbose debug logging — ~29 console.log/time calls across state modules

## Review Notes

The codebase is generally well-structured with consistent patterns:
- Svelte 5 runes used correctly everywhere (no legacy `$:` syntax found)
- No `<style>` blocks (Tailwind inline used throughout as intended)
- State management uses singleton class pattern consistently
- Good TypeScript typing throughout
- Virtual scrolling implemented where needed

The main risk area is the context menu/asset actions duplication (asseteer-mhrg) — any behavior change to folder navigation or file opening would need to be applied in 3 places.
