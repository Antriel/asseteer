---
# asseteer-zycd
title: 'Improve CLAP setup UX: inline progress in settings, better wording'
status: completed
type: task
priority: normal
created_at: 2026-03-21T07:22:10Z
updated_at: 2026-03-21T07:23:14Z
---

Remove the ClapSetupDialog modal and embed setup progress inline in the settings page. Improve wording: the startup-process/waiting-for-server hint should say 'first run downloads ~3-8 GB' rather than 'may take 20+ min'. Remove misleading 'Loading AI model ~1-2 GB' hint. Show log detail (startupDetail) as a progress line during download. Remove the 'Keep this app open' modal note.

## Summary of Changes

- Deleted  (modal no longer needed)
- Rewrote CLAP section in :
  - Setup progress is now inline in the settings card, not a modal overlay
  - 'Keep this app open' notice moved inline (only shown on first-time setup)
  - Wording improvements: 'Setting up Python environment' with hint 'first run downloads ~3–8 GB' replaces misleading '20+ min' and '~1-2 GB' hints
  - 'Downloading package manager' replaces 'Downloading runtime tools'
  - 'Loading model' (no size hint) replaces 'Loading AI model (~1-2 GB first time)'
  - Log tail (startupDetail) shown in a monospace code block during download phase
  - stepStatus() simplified: no longer needs error/ready handling since the if/else blocks handle those states
