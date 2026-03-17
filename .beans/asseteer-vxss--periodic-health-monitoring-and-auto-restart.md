---
# asseteer-vxss
title: Periodic health monitoring and auto-restart
status: todo
type: task
priority: normal
created_at: 2026-03-17T10:05:42Z
updated_at: 2026-03-17T10:06:12Z
parent: asseteer-5kja
blocked_by:
    - asseteer-4eb1
---

Monitor CLAP server health and recover from crashes.

- [ ] Add periodic health check (every 30s) in clap.svelte.ts when server is supposed to be running
- [ ] Detect server crash and update UI state to "Offline"
- [ ] Auto-restart server on crash (with backoff — max 3 retries)
- [ ] Show toast notification when server crashes and is restarted
