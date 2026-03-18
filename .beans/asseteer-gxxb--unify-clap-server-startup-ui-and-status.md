---
# asseteer-gxxb
title: Unify CLAP server startup UI and status
status: completed
type: feature
priority: normal
created_at: 2026-03-18T08:35:29Z
updated_at: 2026-03-18T08:39:27Z
---

Unify CLAP server status across Settings, Processing, and StatusBar views. Add backend progress events, replace time-based fake progress with real phases, and sync setupStatus from all startup paths.

## Summary of Changes

### Backend (Rust)
- **`src-tauri/src/clap/mod.rs`**: Added global `AppHandle` storage so the CLAP module can emit Tauri events without threading `AppHandle` through every function
- **`src-tauri/src/clap/server.rs`**: Added `ClapStartupProgress` event struct and `emit_startup_progress()` helper. Emits real progress events at each phase: `checking`, `downloading-uv` (only if uv not cached), `starting-process`, `waiting-for-server`, `loading-model`, `ready`, `error`
- **`src-tauri/src/commands/search.rs`**: Added `check_clap_setup_state` command that checks disk for uv binary and cache existence
- **`src-tauri/src/lib.rs`**: Initialize AppHandle in setup, register new command

### Frontend
- **`src/lib/state/clap.svelte.ts`**: 
  - Added `startupPhase` and `startupDetail` reactive state
  - Added `listenForStartupEvents()` that listens to backend `clap-startup-progress` events
  - **Fixed root cause**: `ensureServer()` now updates `setupStatus` (was only done in `setup()`), fixing settings staying "Not set up" when server started from search/processing
  - `setup()` now delegates to `ensureServer()` (no duplication)
  - `initialize()` uses `check_clap_setup_state` to distinguish "never set up" from "configured but offline"
- **`src/lib/components/ClapSetupDialog.svelte`**: Replaced time-based fake progress with event-driven real phases. Only shows "Downloading runtime tools" step when uv isn't installed. Adapts title for first-time vs restart.
- **`src/lib/components/ClapProcessingCard.svelte`**: Shows phase-specific startup text instead of generic "Starting CLAP server (loading model)..."
- **`src/lib/components/layout/StatusBar.svelte`**: Shows actual startup detail from backend events instead of hardcoded "Starting..."
- **`src/routes/(app)/settings/+page.svelte`**: Changed "Server stopped unexpectedly" to "Server not running" (more accurate for general offline state)
- **`src/lib/database/queries.ts`**: Added `checkClapSetupState()` invoke wrapper
