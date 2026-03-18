---
# asseteer-3akk
title: Errors not shown in ClapProcessingCard after processing completes
status: completed
type: bug
priority: high
created_at: 2026-03-18T07:53:03Z
updated_at: 2026-03-18T07:54:30Z
---

When CLAP processing finishes with errors, the processing card shows only a green 'Completed' badge with no indication that errors occurred and no way to view them from the card.

The status bar correctly shows 'X failed' and clicking it navigates to the processing tab, but the ClapProcessingCard hides all progress/error details once status === 'completed'.

## Root cause

In `ClapProcessingCard.svelte` (line 239), the progress block (which contains `ProcessingDetails` with the collapsible error list) is gated on:

```svelte
{#if progress && (status === 'running' || status === 'paused')}
```

When processing completes the condition is false so the entire block — including the error count, error list, and Retry Failed button — disappears.

By contrast, `ProcessingCategoryCard.svelte` uses `{:else if progress}` which covers the completed state and correctly keeps errors visible.

## Expected behaviour

After CLAP processing completes with failures:
- The card should show the failed count prominently (e.g. red badge or inline stat)
- The collapsible error list (file + error message) should be accessible
- The Retry Failed button should remain available

## Fix approach

Change the guard in `ClapProcessingCard.svelte` so the progress/errors section also renders when `status === 'completed'` (or more precisely: when `progress` exists and has data), mirroring the pattern in `ProcessingCategoryCard.svelte`. The `ProcessingDetails` component already hides rate/ETA when `isRunning` is false, so it handles the completed state correctly — it just needs to be rendered.

## Summary of Changes

Changed `ClapProcessingCard.svelte` in two places:

1. **Progress block guard** (line 239): Added `|| status === 'completed'` so the progress bar, stats, and `ProcessingDetails` (collapsible error list + Retry Failed button) remain visible after processing finishes.

2. **Status badge** (`statusConfig`): When `status === 'completed'` and `failed > 0`, the badge now shows "Completed with errors" in red instead of green "Completed".
