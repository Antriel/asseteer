---
# asseteer-65ky
title: 'CLAP stop: no ''stopping'' UI state, re-press errors, no feedback during wind-down'
status: completed
type: bug
priority: normal
created_at: 2026-03-23T14:47:59Z
updated_at: 2026-03-23T15:08:29Z
---

When the user clicks Stop on CLAP processing:

1. There is no UI indication that stopping is in progress (e.g. a 'Stopping...' state on the button/card)
2. If the user clicks Stop again while it is winding down, they get an error toast: 'Failed to stop: Processing for category 'clap' is not running'
3. CLAP may take a while to finish in-flight tasks before it fully stops, but the UI gives no feedback during this wind-down period — the user has no idea what is happening

## Expected Behavior
- After clicking Stop, the button/card should enter a 'Stopping...' state (disabled, different label or spinner)
- Re-pressing Stop while stopping should be a no-op or show a gentle message, not an error
- The UI should remain in the stopping state until CLAP is fully stopped


## Summary of Changes

- `tasks.svelte.ts`: Added `stoppingCategories: SvelteSet` to track categories in wind-down after stop is requested. `stop()` adds the category on entry; `updateCategoryProgress()` clears it when `isRunning` goes false. "not running" backend errors are silently ignored (already stopped).
- `ClapProcessingCard.svelte`: Derives `isStopping` from `stoppingCategories`. Status badge switches to amber "Stopping..." while winding down. Stop/Pause/Resume buttons are hidden and replaced with a non-interactive "Stopping..." label until fully stopped.
- `ProcessingCategoryCard.svelte`: Same treatment applied to the generic image/audio card.
