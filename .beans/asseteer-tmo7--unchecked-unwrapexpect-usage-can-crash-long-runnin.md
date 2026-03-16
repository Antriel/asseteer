---
# asseteer-tmo7
title: Unchecked unwrap/expect usage can crash long-running app paths
status: todo
type: bug
priority: normal
created_at: 2026-02-14T07:32:10Z
updated_at: 2026-02-14T07:32:14Z
parent: asseteer-bh0n
---

Multiple runtime paths rely on unwrap/expect in non-test code, including worker processing and similarity sort: src-tauri/src/task_system/work_queue.rs (e.g. lines ~205, ~245, ~299, ~505), src-tauri/src/commands/search.rs:131 (partial_cmp(...).unwrap()), and startup/server path handling in src-tauri/src/lib.rs and src-tauri/src/clap/server.rs. A single unexpected value (e.g., NaN similarity or poisoned timing/path state) can panic and terminate processing/app execution. Replace with fallible handling and explicit error propagation/fallback ordering.
