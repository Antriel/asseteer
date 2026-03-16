---
# asseteer-j66e
title: Scan pipeline is blocking and all-in-memory, risky for large libraries
status: todo
type: bug
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-02-14T07:32:05Z
parent: asseteer-bh0n
---

start_scan calls discover_files synchronously and only then inserts (src-tauri/src/commands/scan.rs lines ~63-70). Discovery walks filesystem/zip recursively (lines ~144-305) and accumulates all assets in a single Vec before DB writes (line ~357). For large folders/zips this can cause long UI stalls and high memory pressure. Scan should stream/chunk discovery+insert and run blocking IO in dedicated blocking tasks.
