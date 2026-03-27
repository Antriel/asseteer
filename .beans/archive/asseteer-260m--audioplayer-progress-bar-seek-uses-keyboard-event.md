---
# asseteer-260m
title: AudioPlayer progress bar seek uses keyboard event as MouseEvent
status: completed
type: bug
priority: normal
created_at: 2026-03-16T09:19:32Z
updated_at: 2026-03-16T14:44:18Z
parent: asseteer-cfrp
---

In AudioPlayer.svelte:303, the progress bar's onkeydown handler casts the KeyboardEvent to MouseEvent via 'seek(e as any)'. The seek function (line 210-216) reads e.clientX to calculate position. A KeyboardEvent has no meaningful clientX (it's 0), so pressing Enter/Space on the progress bar always seeks to time 0. This is a broken accessibility handler — keyboard seek should either use a different calculation or be disabled.

## Summary of Changes

Fixed broken keyboard seek on the AudioPlayer progress bar. The old handler cast a `KeyboardEvent` to `MouseEvent` and called `seek()`, which read `clientX` (always 0 for keyboard events), causing it to always seek to time 0.

**Fix**: Replaced the `Enter`/`Space` → `seek(e as any)` handler with proper `ArrowLeft`/`ArrowRight` keyboard navigation that seeks by 5% of duration in each direction. Also updated the ARIA role from `button` to `slider` with proper `aria-valuemin`, `aria-valuemax`, `aria-valuenow`, and `aria-label` attributes.
