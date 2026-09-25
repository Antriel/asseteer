---
# asseteer-vayv
title: 'CLAP cache clear: stop server first and show progress UI'
status: completed
type: bug
priority: normal
created_at: 2026-04-09T15:25:04Z
updated_at: 2026-09-25T12:38:37Z
---

Clearing CLAP cache fails with 'still in use' error because the server is still running. Should stop the server before clearing cache. Also missing UI feedback for clear-in-progress and clear-done states.

## Summary of Changes

**Root cause:** the server was already being stopped before the clear (`393119f`), but `child.kill()` only killed the `uv.exe` launcher. The real server is a `python.exe` grandchild. It kept running and kept the uv cache locked, along with the port and the GPU. Confirmed by spawning `uv run` and killing uv: python survived.

- `clap/job_object.rs`: the process-wide job object is replaced by `ProcessTree`, a per-server Job Object (KILL_ON_JOB_CLOSE). `kill_and_wait` calls `TerminateJobObject` and then polls `ActiveProcesses` until the whole tree has exited, with a 10s timeout. A Windows test checks that grandchildren die; it fails with the old kill-root-only behaviour.
- `stop_server_and_wait` returns a `Result`. `clear_clap_cache` returns an error if the stop fails, and deletes on `spawn_blocking`.
- A failed server start now kills its tree and clears the slot. Before, a dead process stayed in the slot and counted as running.
- UI: `clapState.clearingCache` drives a "Clearing..." spinner, and the Clear button is disabled while stopping, setting up or during CLAP processing. The cache size is refreshed after a failed clear. When the clear is done, a toast appears and the section goes back to "Not set up".

Not verified against a real CLAP server (the harness can't run one). That needs a manual check.
