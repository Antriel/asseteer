---
# asseteer-40vq
title: Audio processing start has no feedback before progress begins
status: completed
type: bug
priority: normal
created_at: 2026-03-24T12:32:22Z
updated_at: 2026-03-25T11:40:23Z
---

When clicking to start audio processing, there's no immediate feedback until progress starts (can take ~1 second). Also, the 'Processing: <filepath>' line sometimes disappears (shows null) — should show last entry instead of hiding the line to avoid UI jitter.


## Root Cause Analysis: Flaky Current File Display

The `current_file` field in `CategoryState` is a single `RwLock<Option<String>>` shared by **all workers** (23 on this machine). Every worker writes `Some(path)` before processing and `None` after. With 23 workers concurrently stomping on the same field:

1. **Overwrites**: Worker A sets `Some("file_a.mp3")`, Worker B immediately overwrites with `Some("file_b.mp3")`
2. **Premature clears**: Worker A finishes and writes `None`, clearing the value even while Workers B-W are mid-processing
3. **Tiny window**: The progress emitter polls every 2s but the `Some(...)` window per asset is ~50-200ms, and workers spend most time between assets (dequeuing, locking)

The rusqlite DbBatchWriter change made this worse — faster writes mean assets complete more quickly, shrinking the `Some` window further.

### Fix approach
Either per-worker `current_file` (progress emitter picks any non-None), or stop writing `None` after processing (overwrite with next file instead, only clear on idle/stop).

## Summary of Changes

**Backend** (`src-tauri/src/task_system/work_queue.rs`):
1. **Immediate feedback on start**: Emit an initial `processing-progress` event right when processing begins (before the 2s ticker loop), so the UI updates instantly.
2. **Stop clearing `current_file` after each asset**: Removed the `*current = None` writes after processing each individual asset and each CLAP batch. Workers now just overwrite with the next file path, keeping a valid value visible to the progress emitter at all times.
3. **Clear `current_file` on completion/stop**: Added explicit `None` writes in the completion path and stop handler so stale values don't linger after processing ends.

**Frontend** (`src/lib/components/ProcessingDetails.svelte`):
4. **"Starting..." placeholder**: When processing is running but no file has been reported yet, show "Starting..." instead of hiding the row entirely. This gives immediate visual feedback.
