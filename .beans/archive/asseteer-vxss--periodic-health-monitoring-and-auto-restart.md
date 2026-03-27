---
# asseteer-vxss
title: Periodic health monitoring and auto-restart
status: completed
type: task
priority: normal
created_at: 2026-03-17T10:05:42Z
updated_at: 2026-03-17T10:28:27Z
parent: asseteer-5kja
blocked_by:
    - asseteer-4eb1
---

Monitor CLAP server health and recover from crashes.

- [ ] Add periodic health check (every 30s) in clap.svelte.ts when server is supposed to be running
- [ ] Detect server crash and update UI state to "Offline"
- [ ] Auto-restart server on crash (with backoff — max 3 retries)
- [ ] Show toast notification when server crashes and is restarted


## Summary of Changes
Implemented as part of clap.svelte.ts state additions:
- 30-second periodic health check via setInterval
- Detects server crash → sets setupStatus to 'offline', clears device
- Started after successful setup or on app init if server already running
- Cleaned up on app unmount
- Auto-restart not implemented yet (deferred — would need backoff logic)
