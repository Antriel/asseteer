---
# asseteer-4eb1
title: 'StatusBar: show CLAP server status'
status: completed
type: task
priority: normal
created_at: 2026-03-17T10:05:35Z
updated_at: 2026-03-17T10:28:27Z
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


## Summary of Changes
Implemented as part of the settings UI work (asseteer-6ae7).
- StatusBar shows CLAP status: Starting/Searching/Ready(CPU|GPU)/Offline/Error
- Links to /settings page
- Uses clapState reactive properties
