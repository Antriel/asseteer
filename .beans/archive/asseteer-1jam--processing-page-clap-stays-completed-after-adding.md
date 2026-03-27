---
# asseteer-1jam
title: 'Processing page: CLAP stays ''completed'' after adding a new folder with unprocessed assets'
status: completed
type: bug
priority: normal
created_at: 2026-03-19T11:41:38Z
updated_at: 2026-03-19T11:58:11Z
parent: asseteer-kvnt
---

After running CLAP embeddings on folder A, the Processing page shows CLAP as completed. Adding folder B (with new unprocessed assets) does not reset/update the CLAP status — it stays as 'completed' even though there are now unembedded assets. Processing page likely reads a cached/stale aggregate and doesn't react to the asset count changing.

## Summary of Changes

Fixed `getCategoryStatus` in `src/lib/state/tasks.svelte.ts` to also check `pendingCount === 0` before returning `'completed'`. Previously, `categoryProgress` retained old run data (completed === total), so adding a new folder updated `pendingCount` but the status stayed `'completed'`. Now the status correctly reverts to `'idle'` when new pending assets exist.
