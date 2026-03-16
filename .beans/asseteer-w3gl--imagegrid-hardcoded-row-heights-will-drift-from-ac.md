---
# asseteer-w3gl
title: ImageGrid hardcoded row heights will drift from actual rendered size
status: todo
type: bug
priority: low
created_at: 2026-03-16T09:19:48Z
updated_at: 2026-03-16T09:19:48Z
parent: asseteer-cfrp
---

ImageGrid.svelte:40-46 hardcodes row heights (e.g., 192 + 56 + 8 for medium). These values are based on Tailwind class assumptions (h-48 = 192px, metadata ~56px, gap-2 = 8px). If the font size, metadata content, or Tailwind config changes, the virtual scroll offset calculations will be wrong — items will overlap or have gaps. The VirtualList component has the same issue but with externally-provided itemHeight, putting the burden on callers. Consider measuring actual row height dynamically.
