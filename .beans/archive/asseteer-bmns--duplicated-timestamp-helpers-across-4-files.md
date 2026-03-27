---
# asseteer-bmns
title: Duplicated timestamp helpers across 4 files
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:44:52Z
updated_at: 2026-03-21T08:42:09Z
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

## Summary of Changes

- Added `pub fn unix_now() -> i64` and `pub fn now_millis() -> u64` to `utils.rs`
- Removed local `unix_now()` from `task_system/work_queue.rs` and `task_system/processor.rs`; both now import from `crate::utils`
- Removed local `now_millis() -> u64` from `thumbnail_worker.rs`; now imports from `crate::utils`
- Removed local `now_millis() -> u128` from `commands/rescan.rs`; now uses `crate::utils::now_millis()` (u64, sufficient for format string use)
