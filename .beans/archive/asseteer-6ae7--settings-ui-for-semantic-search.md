---
# asseteer-6ae7
title: Settings UI for semantic search
status: completed
type: task
priority: normal
created_at: 2026-03-17T10:05:31Z
updated_at: 2026-03-17T10:28:27Z
parent: asseteer-5kja
blocked_by:
    - asseteer-syol
---

Add semantic search configuration to settings page.

- [ ] Add settings route/page if it doesn't exist yet
- [ ] Semantic Search toggle (enable/disable)
- [ ] Server status indicator: Not set up / Setting up / Ready / Running
- [ ] Runtime info: Python version, device (CPU/GPU), cache size
- [ ] Manual Python path override for fallback users
- [ ] Clear Cache button (removes downloaded Python/packages, shows size)
- [ ] Wire settings to Rust backend (store in app config)


## Summary of Changes

### Rust backend
- Added `health_check_detailed()` to `client.rs` — returns `HealthInfo` (status, model, device, embedding_dim)
- Added 3 new Tauri commands in `search.rs`: `get_clap_server_info`, `get_clap_cache_size`, `clear_clap_cache`
- Registered in `lib.rs` invoke handler

### Frontend queries
- Added `getClapServerInfo()`, `getClapCacheSize()`, `clearClapCache()` to `queries.ts`
- Added `ClapServerInfo` TypeScript interface

### CLAP state (`clap.svelte.ts`)
- Added `device`, `model`, `setupStatus`, `setupError`, `cacheSize` state fields
- Added `setup()`, `refreshServerInfo()`, `refreshCacheSize()`, `clearCache()` methods
- Added `startHealthMonitor()` / `stopHealthMonitor()` — 30s periodic checks
- Added `initialize()` — called once on app mount, checks server state + cache size

### Settings page (`src/routes/(app)/settings/+page.svelte`)
- New route with Semantic Search section showing: status indicator, setup/retry button, device/model info, cache size + clear button
- About section with version

### Sidebar
- Added Settings link at bottom of sidebar with gear icon
- Separate `bottomNavItems` array for bottom-pinned nav

### StatusBar
- Added CLAP server status indicator: Starting (yellow pulse) / Searching (blue pulse) / Ready+device (green) / Offline (gray) / Error (red)
- Links to /settings for details

### Root layout
- Initializes CLAP state on mount, cleans up health monitor on unmount
