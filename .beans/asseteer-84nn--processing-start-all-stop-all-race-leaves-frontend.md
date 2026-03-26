---
# asseteer-84nn
title: Processing Start All -> Stop All race leaves frontend state inconsistent
status: completed
type: bug
priority: high
created_at: 2026-03-26T08:06:05Z
updated_at: 2026-03-26T09:36:11Z
---

## Summary

Start All can race with Stop All on the Processing page. If the user clicks Start All and then Stop All before every category has finished backend startup, the frontend can skip stopping categories that are still in the local `starting` state. Those categories can then become active after the user already pressed Stop, leaving the UI in a bad state.

## User-visible symptom

On `src/routes/(app)/processing/+page.svelte`, the user can:
1. Click Start All.
2. Quickly click Stop All while one or more categories are still preparing.
3. End up with processing UI/state that does not match the user's intent to stop everything.

Observed/likely symptoms:
- A category that looked like it was part of Start All is not actually stopped.
- Stop All returns the page to a partly-idle state, then a category flips back into running once backend startup finishes.
- Per-category status chips/buttons can get out of sync with the user's stop action.
- Pending counts and last-run summary can become misleading until a later refresh/event corrects them.

## Primary bug: frontend skips categories that are still "starting"

Relevant frontend code:
- `src/lib/state/tasks.svelte.ts:131-187` (`startProcessing`)
- `src/lib/state/tasks.svelte.ts:193-207` (`startAllEnabled`)
- `src/lib/state/tasks.svelte.ts:287-295` (`stopAll`)

Details:
- `startProcessing(category)` immediately does optimistic frontend state updates:
  - adds the category to `startingCategories`
  - inserts a synthetic `categoryProgress` row with `is_running: false` / `isRunning: false`
- `startAllEnabled()` launches `startProcessing()` for each enabled category in parallel.
- `stopAll()` only selects categories where `progress.isRunning` is already true:
  - `Array.from(this.categoryProgress.entries()).filter(([_, progress]) => progress.isRunning)`
- That means categories still in the `startingCategories` set but not yet confirmed as `isRunning` are completely ignored by Stop All.

This creates a race window:
- Start All begins.
- Frontend marks categories as "Starting..." but not running yet.
- User presses Stop All during that window.
- `stopAll()` only stops categories already marked running.
- Skipped categories continue their startup path and may become running after Stop All has finished.

## Why the startup window exists

Relevant backend code:
- `src-tauri/src/commands/process.rs:8-88` (`start_processing`)
- `src-tauri/src/task_system/work_queue.rs:364-589` (`WorkQueue::start`)
- `src-tauri/src/task_system/work_queue.rs:833-847` (initial progress emit)

Details:
- `start_processing` does non-trivial work before the queue is considered running:
  - parse category
  - query all pending assets from SQLite
  - only then call `work_queue.start(...)`
- `WorkQueue::start` is where `state.is_running` is atomically switched to `true`.
- The first explicit progress event with `is_running: true` is emitted later by the progress emitter.

So there is a real backend preparation period where:
- the frontend already thinks the category is "starting"
- but backend `stop_processing` is not necessarily available yet, because `is_running` has not been set
- and frontend Stop All currently does nothing for that category anyway

## Secondary backend bug discovered during investigation

Relevant backend code:
- `src-tauri/src/task_system/work_queue.rs:128-140` (`dispatcher_handle` field)
- `src-tauri/src/task_system/work_queue.rs:411-563` (store dispatcher handle during `start`)
- `src-tauri/src/task_system/work_queue.rs:982-985` (abort dispatcher during `stop`)

`WorkQueue` has a single global `dispatcher_handle: Arc<RwLock<Option<JoinHandle<()>>>>`, not one per category.

Implication:
- Starting multiple categories in parallel can overwrite the stored dispatcher handle.
- Stopping one category aborts whichever staged dispatcher was stored last, not necessarily the dispatcher for the category being stopped.

This may not be the first bug to fix for the reported symptom, but it is likely to make Start All / Stop All interactions less deterministic, especially when multiple categories with ZIP-group dispatch are active.

## Likely fix direction

Do not implement from this bean without re-checking current code, but likely options are:
- Frontend: treat `startingCategories` as stoppable/cancelable in `stopAll()` and per-category stop logic.
- Frontend: preserve a stronger user intent flag like "requested stop while starting" so the category is stopped/cancelled as soon as backend start completes.
- Backend: support cancellation during startup/preparation, or expose a state that lets the frontend stop categories that are still preparing.
- Backend: make staged dispatcher tracking per-category instead of one global handle.

## Repro notes

Most likely to reproduce when startup preparation is slow enough to widen the gap before first progress update:
- many pending assets
- audio/CLAP categories
- ZIP/nested-ZIP heavy datasets
- Start All across multiple categories, then Stop All immediately

## Open questions for fix session

- Should Stop All cancel categories in `startingCategories` before backend reports them as running?
- Should the frontend hide Stop All until startup confirmation, or should it honor the user's stop intent immediately?
- Does backend `stop_processing` need a preparatory/cancel-pending state for categories that have not yet reached `is_running = true`?
- Is the single global `dispatcher_handle` already causing cross-category interference in other processing flows?

## Suggested todo list

- [x] Reproduce the Start All -> immediate Stop All race intentionally.
- [x] Decide the desired contract for stopping categories that are still starting.
- [x] Fix frontend state logic so Stop All includes categories in startup transition.
- [x] Fix backend staged-dispatcher ownership so stop is category-scoped.
- [x] Verify no stuck UI state remains after rapid start/stop interactions.

## Summary of Changes

### Frontend (src/lib/state/tasks.svelte.ts)
- Added `pendingStopCategories` SvelteSet to track stop intent for categories still starting
- Modified `stop()` to queue stop for categories in `startingCategories` instead of calling backend (which would fail with "not running")
- Modified `stopAll()` to also queue stops for all categories in `startingCategories`
- Modified `startProcessing()` to check `pendingStopCategories` after backend startup completes and immediately stop if flagged

### Backend (src-tauri/src/task_system/work_queue.rs)
- Changed `dispatcher_handle` from single global `Option<JoinHandle>` to per-category `HashMap<ProcessingCategory, JoinHandle>`
- `start()` now stores dispatcher handle keyed by category
- `stop()` now only aborts the dispatcher for the specific category being stopped, not whichever was stored last
