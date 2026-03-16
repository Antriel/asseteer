---
# asseteer-wih9
title: totalMatchingCount is always result.length regardless of truncation
status: completed
type: bug
priority: high
created_at: 2026-03-16T09:18:46Z
updated_at: 2026-03-16T14:38:38Z
parent: asseteer-cfrp
---

In assets.svelte.ts:80, both branches of the ternary are identical: result.length > MAX_DISPLAY_LIMIT ? result.length : result.length. When results are truncated to 5000, totalMatchingCount shows 5001 instead of the actual count of matching rows in the DB. The empty-state message ('You have X images') also uses this value, so it shows wrong counts. Fix: run a separate COUNT query or use the DB's total_changes to get the true count.

## Summary of Changes

- Added `countSearchResults()` query in `queries.ts` that mirrors `searchAssets` filters but returns a COUNT
- Fixed `assets.svelte.ts` line 80: when results exceed MAX_DISPLAY_LIMIT, runs the count query to get the true total instead of using `result.length` (which was always 5001)
- Non-truncated results still use `result.length` directly (no extra query needed)
