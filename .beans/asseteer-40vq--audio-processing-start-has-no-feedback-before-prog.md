---
# asseteer-40vq
title: Audio processing start has no feedback before progress begins
status: todo
type: bug
priority: normal
created_at: 2026-03-24T12:32:22Z
updated_at: 2026-03-25T07:15:04Z
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
