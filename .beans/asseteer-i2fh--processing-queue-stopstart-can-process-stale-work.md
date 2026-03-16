---
# asseteer-i2fh
title: Processing queue stop/start can process stale work and corrupt progress
status: todo
type: bug
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-02-14T07:32:05Z
parent: asseteer-bh0n
---

src-tauri/src/task_system/work_queue.rs resets stop_signal on start (line ~122) while a shared queue may still contain previously queued items. Workers gate only on current stop flag when popping items (lines ~171-199), so old items can be processed after a restart, mixing runs and corrupting counters/completion behavior (line ~349). This risks duplicate processing and incorrect progress/state transitions.
