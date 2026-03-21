---
# asseteer-qdes
title: Clean up verbose debug logging in state modules
status: todo
type: task
priority: low
created_at: 2026-03-20T11:44:17Z
updated_at: 2026-03-20T11:44:17Z
parent: asseteer-38rb
---

Several state modules have extensive console.log/console.time statements that were useful during development but should be cleaned up for release:

- `src/lib/state/tasks.svelte.ts` — 18 console.log/time/timeEnd calls
- `src/lib/state/clap.svelte.ts` — 4 console.log calls (ensureServer debug logging)
- `src/routes/(app)/library/+page.svelte` — 7 console.log/time/timeEnd calls

These add noise to the browser console in production. Consider removing or gating behind a debug flag.
