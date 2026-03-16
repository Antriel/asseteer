---
# asseteer-fkqr
title: ImageGrid columnCount doesn't update on window resize
status: todo
type: bug
priority: normal
created_at: 2026-03-16T09:18:51Z
updated_at: 2026-03-16T09:22:50Z
parent: asseteer-cfrp
---

In ImageGrid.svelte:27-37, columnCount is a $derived that reads window.innerWidth at evaluation time. But there's no reactive signal for window width — the derived value is computed once and never re-evaluated when the window resizes. The ResizeObserver only updates containerHeight, not columnCount. This means if a user resizes the window across the 1280px XL breakpoint, the grid column count goes stale while the CSS grid-cols class (driven by Tailwind responsive prefixes) updates correctly, causing a mismatch between virtual scroll calculations and actual layout. Items will be positioned incorrectly or disappear.
