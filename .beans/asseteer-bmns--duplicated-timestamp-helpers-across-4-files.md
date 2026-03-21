---
# asseteer-bmns
title: Duplicated timestamp helpers across 4 files
status: todo
type: task
priority: normal
created_at: 2026-03-20T11:44:52Z
updated_at: 2026-03-20T11:48:59Z
parent: asseteer-c0lx
---

unix_now() is defined identically in:
- `src/task_system/work_queue.rs:19`
- `src/task_system/processor.rs:16`

And near-identical `now_millis()` variants in:
- `src/thumbnail_worker.rs:422` (returns u64)
- `src/commands/rescan.rs:462` (returns u128)

All compute `SystemTime::now().duration_since(UNIX_EPOCH)` with minor return type differences.

**Fix**: Extract a single `unix_now() -> i64` into a shared module (e.g., `utils.rs`) and replace all copies.


---
**CLAUDE.md**: If a shared timestamp helper is added to `utils.rs`, no CLAUDE.md update needed — `utils.rs` is already documented as containing utility functions.
