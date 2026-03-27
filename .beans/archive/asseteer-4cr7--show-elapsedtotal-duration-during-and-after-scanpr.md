---
# asseteer-4cr7
title: Show elapsed/total duration during and after scan/processing
status: completed
type: feature
priority: low
created_at: 2026-03-24T12:32:28Z
updated_at: 2026-03-25T11:47:31Z
---

During scanning and processing, show elapsed time alongside the ETA. When the operation completes, display how long it took in the UI. Small quality-of-life detail that's satisfying for the user.


## Summary of Changes

Added elapsed time display during scan and processing operations, plus total duration when complete.

### State changes:
- `ui.svelte.ts`: Added `scanStartedAt`, `scanDurationMs`, `startScanTimer()`, `stopScanTimer()` to UIState
- `tasks.svelte.ts`: Added `categoryStartedAt`/`categoryDurationMs` SvelteMaps, `processingStartedAt`, `durationMs` to `ProcessingRunResult`, and `formatElapsed(ms)` utility

### UI changes:
- **ProcessingDetails.svelte**: Shows live "Elapsed: Xm Ys" alongside Rate and ETA when processing
- **ProcessingCategoryCard.svelte**: Shows "in Xm Ys" next to "Completed" badge when a category finishes
- **StatusBar.svelte**: Shows elapsed time during scan/processing, and "in Xm Ys" in the "Complete" state
- **Folders page**: Shows elapsed time alongside scan progress indicator
- **ScanControl.svelte**: Calls start/stop scan timer
