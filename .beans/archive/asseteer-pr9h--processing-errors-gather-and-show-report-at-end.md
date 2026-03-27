---
# asseteer-pr9h
title: 'Processing errors: gather and show report at end'
status: completed
type: feature
priority: normal
created_at: 2026-03-16T11:42:37Z
updated_at: 2026-03-16T15:13:35Z
---

Errors during processing should be collected and shown to the user in a final report view after processing completes (not just logged), since there could be many.


## Approach

Show processing results in the status bar after completion instead of reverting to "Idle":
- Add `lastRunResult` state to track completed/failed counts after processing finishes
- StatusBar shows "Complete: X processed, Y errors" (or just "X processed" if no errors)
- Error count links to processing page for details
- Result clears when new processing starts

## Tasks

- [x] Add `lastRunResult` state to `ProcessingState` in `tasks.svelte.ts`
- [x] Set result on completion events, clear on start
- [x] Update StatusBar to show completion results


## Summary of Changes

- Added `ProcessingRunResult` interface and `lastRunResult` state to `ProcessingState`
- `checkAllComplete()` sets the result when all categories finish (aggregates total/completed/failed)
- Result cleared when `startProcessing()` is called
- StatusBar shows "Complete: X processed" (green dot) or "Complete: X processed, Y failed" (red dot) after processing finishes
- Clicking the status navigates to the processing page for error details
- Category progress bars remain visible alongside the summary
