---
# asseteer-e9cn
title: 'CLAP: disable semantic button when not configured, link to settings'
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:38:43Z
updated_at: 2026-03-20T11:39:50Z
---

When CLAP is not yet set up (not-configured), disable the Semantic button in the toolbar instead of auto-starting setup. Clicking it should navigate to Settings. Only auto-start when previously set up (offline/error status).


## Summary of Changes

**`src/lib/state/clap.svelte.ts`:**
- Added `setupKnown = $state(false)` — set to `true` at the end of `initialize()` to avoid a false "not-configured" flash while the async init is running

**`src/lib/components/shared/Toolbar.svelte`:**
- Added `goto` import from `$app/navigation`
- Added `clapNotConfigured` derived: `clapState.setupKnown && clapState.setupStatus === 'not-configured'`
- `toggleSemanticSearch()` short-circuits to `goto('/settings')` when not configured
- Button: shows `opacity-50` + gear icon + tooltip "Semantic search requires one-time setup — click to go to Settings" when not configured; otherwise behaves exactly as before
