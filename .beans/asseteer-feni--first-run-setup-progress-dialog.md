---
# asseteer-feni
title: First-run setup progress dialog
status: completed
type: task
priority: high
created_at: 2026-03-17T10:05:27Z
updated_at: 2026-03-17T10:28:27Z
parent: asseteer-5kja
blocked_by:
    - asseteer-syol
---

Show a multi-step progress dialog when user first enables semantic search.

- [ ] Create `ClapSetupDialog.svelte` component
- [ ] Show 3 steps: downloading runtime tools (uv), installing Python environment, downloading AI model
- [ ] Wire to Tauri events for progress updates from Rust
- [ ] Add cancel button that aborts setup
- [ ] Show "one-time setup" messaging
- [ ] On completion, transition to "Ready" state


## Summary of Changes

### ClapSetupDialog.svelte
- Modal dialog shown when user clicks "Set Up" in settings
- 3-step progress display: downloading tools → installing Python → downloading AI model
- Steps derived from elapsed time (since uv subprocess doesn't report granular progress)
- Animated spinner for active step, checkmarks for completed, X for errors
- Error display with retry button
- Cancel button throughout, auto-closes on success after 800ms
- Validated with Svelte autofixer — uses $derived.by, keyed #each, proper Props interface
