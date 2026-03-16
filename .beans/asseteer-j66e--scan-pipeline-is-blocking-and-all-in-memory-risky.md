---
# asseteer-j66e
title: Scan pipeline is blocking and all-in-memory, risky for large libraries
status: completed
type: bug
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-03-16T11:56:20Z
parent: asseteer-bh0n
---

start_scan calls discover_files synchronously and only then inserts (src-tauri/src/commands/scan.rs lines ~63-70). Discovery walks filesystem/zip recursively (lines ~144-305) and accumulates all assets in a single Vec before DB writes (line ~357). For large folders/zips this can cause long UI stalls and high memory pressure. Scan should stream/chunk discovery+insert and run blocking IO in dedicated blocking tasks.

## Summary of Changes

Rewrote the scan pipeline from blocking/all-in-memory to streaming/concurrent:

**Backend (`scan.rs`)**:
- Discovery now runs on a dedicated blocking thread via `tokio::task::spawn_blocking`
- Assets are streamed in chunks of 200 through a `tokio::sync::mpsc` channel instead of accumulating in a single `Vec`
- DB insertion runs concurrently on the async runtime, inserting chunks as they arrive from discovery
- Each chunk is inserted in its own transaction (vs one giant transaction for everything)
- Zip archive scanning also streams chunks, flushing after each zip file
- Progress events emitted from both discovery and insertion sides

**Frontend**:
- Added new `scanning` phase that shows concurrent discovery + insertion progress
- During concurrent phase: shows "found" and "saved" counters side by side
- Once discovery completes: transitions to progress bar showing remaining insertions
- Updated `ScanControl.svelte`, scan page, and `ScanProgress` interface
