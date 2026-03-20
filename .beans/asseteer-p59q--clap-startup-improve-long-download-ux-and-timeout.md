---
# asseteer-p59q
title: 'CLAP startup: improve long-download UX and timeout handling'
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:34:45Z
updated_at: 2026-03-20T11:36:49Z
---

The 120s timeout is too short for GPU torch downloads (~8GB). Error message incorrectly suggests restarting which cancels the download. Need: longer timeout, process-liveness fast-fail, log tailing for progress, and better messaging.


## Summary of Changes

**`src-tauri/src/clap/server.rs`:**
- `start_server_process` now returns `(Child, PathBuf)` — log path threaded through to `wait_for_server_ready`
- `wait_for_server_ready(child, log_path)` — 30-min timeout (was 120s), checks `child.try_wait()` on every iteration for fast-fail on process death, tails log every 10s and emits last meaningful uv/pip line as `startupDetail`
- Added `read_log_last_meaningful_line` — reads last 4KB, prefers lines matching uv/pip keywords, truncates to 80 chars
- Added `read_log_tail` — reads last 2KB for error messages on process exit
- Error message updated: no longer says "restart the app"; explains keeping the app open during download

**`src/lib/components/ClapSetupDialog.svelte`:**
- "Starting Python server" step now shows `may take 20+ min (GPU: ~8 GB)` hint on first-time setup
- Added "Keep this app open while downloading" notice during the waiting phase on first-time setup
