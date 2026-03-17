---
# asseteer-4eb1
title: 'StatusBar: show CLAP server status'
status: todo
type: task
priority: normal
created_at: 2026-03-17T10:05:35Z
updated_at: 2026-03-17T10:06:12Z
parent: asseteer-5kja
blocked_by:
    - asseteer-6ae7
---

Add CLAP server status indicator to the status bar.

- [ ] Show states: Setup required (gray) / Offline (gray) / Starting (yellow) / Ready+device (green) / Searching (blue)
- [ ] Enhance health check to parse and return device info (CPU/GPU)
- [ ] Add `health_check_detailed()` to client.rs returning HealthInfo struct
- [ ] Expose as Tauri command
- [ ] Add `device` field to ClapState in clap.svelte.ts
- [ ] Update StatusBar.svelte with CLAP status section
