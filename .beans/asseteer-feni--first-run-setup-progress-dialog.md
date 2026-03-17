---
# asseteer-feni
title: First-run setup progress dialog
status: todo
type: task
priority: high
created_at: 2026-03-17T10:05:27Z
updated_at: 2026-03-17T10:06:12Z
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
