---
# asseteer-x0ei
title: Capture CLAP server logs to file
status: completed
type: task
priority: low
created_at: 2026-03-17T10:05:48Z
updated_at: 2026-03-17T11:16:24Z
parent: asseteer-5kja
blocked_by:
    - asseteer-syol
---

Redirect Python server output to a log file for troubleshooting.

- [x] Pipe Python process stdout/stderr to log file in app data dir
- [x] Rotate logs (keep last 5 runs)
- [x] Add "View Logs" button in settings UI
- [x] Include log path in error messages for debugging
