---
# asseteer-tmo7
title: Unchecked unwrap/expect usage can crash long-running app paths
status: completed
type: bug
priority: normal
created_at: 2026-02-14T07:32:10Z
updated_at: 2026-03-16T11:59:41Z
parent: asseteer-bh0n
---

Multiple runtime paths rely on unwrap/expect in non-test code, including worker processing and similarity sort: src-tauri/src/task_system/work_queue.rs (e.g. lines ~205, ~245, ~299, ~505), src-tauri/src/commands/search.rs:131 (partial_cmp(...).unwrap()), and startup/server path handling in src-tauri/src/lib.rs and src-tauri/src/clap/server.rs. A single unexpected value (e.g., NaN similarity or poisoned timing/path state) can panic and terminate processing/app execution. Replace with fallible handling and explicit error propagation/fallback ordering.

## Summary of Changes

- **search.rs:131** (CRITICAL): Replaced `partial_cmp().unwrap()` with `total_cmp()` for float sorting — NaN values no longer cause a panic
- **work_queue.rs:214** (CRITICAL): Replaced `semaphore.acquire().unwrap()` with `let Ok(_permit) = ... else { break }` — closed semaphore no longer panics the worker
- **work_queue.rs + processor.rs** (7 occurrences): Replaced all `SystemTime::duration_since(UNIX_EPOCH).unwrap()` with a `unix_now()` helper using `unwrap_or_default()` — clock misconfiguration returns epoch 0 instead of panicking
