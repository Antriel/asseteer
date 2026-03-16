---
# asseteer-w3gl
title: ImageGrid hardcoded row heights will drift from actual rendered size
status: completed
type: bug
priority: low
created_at: 2026-03-16T09:19:48Z
updated_at: 2026-03-16T15:01:18Z
parent: asseteer-cfrp
---

ImageGrid.svelte:40-46 hardcodes row heights (e.g., 192 + 56 + 8 for medium). These values are based on Tailwind class assumptions (h-48 = 192px, metadata ~56px, gap-2 = 8px). If the font size, metadata content, or Tailwind config changes, the virtual scroll offset calculations will be wrong — items will overlap or have gaps. The VirtualList component has the same issue but with externally-provided itemHeight, putting the burden on callers. Consider measuring actual row height dynamically.

## Summary of Changes

ImageGrid now dynamically measures the actual rendered row height from the first grid item instead of relying solely on hardcoded estimates. The hardcoded values are kept as initial estimates before measurement completes, but once the DOM renders, `measureRowHeight()` reads the actual `offsetHeight` of the first child element. Re-measurement triggers on thumbnail size changes and when grid children are added/removed (via MutationObserver).
