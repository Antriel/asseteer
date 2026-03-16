---
# asseteer-i2fh
title: Processing queue stop/start can process stale work and corrupt progress
status: completed
type: bug
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-03-16T11:46:50Z
parent: asseteer-bh0n
---

src-tauri/src/task_system/work_queue.rs resets stop_signal on start (line ~122) while a shared queue may still contain previously queued items. Workers gate only on current stop flag when popping items (lines ~171-199), so old items can be processed after a restart, mixing runs and corrupting counters/completion behavior (line ~349). This risks duplicate processing and incorrect progress/state transitions.

## Summary of Changes\n\nAdded a generation counter to \ in \. Each call to \ increments the generation and stamps all queued work items. Workers skip items whose generation doesn't match the current one, preventing stale items from a previous run from being processed after a stop/restart cycle.
