---
# asseteer-xfu1
title: Deduplicate query filter building in queries.ts
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:43:33Z
updated_at: 2026-03-21T07:55:26Z
parent: asseteer-38rb
---

In `src/lib/database/queries.ts`, `searchAssets()` (lines 70-121) and `countSearchResults()` (lines 126-171) build nearly identical filter conditions:

- Both call buildFtsCondition() for search text
- Both add asset_type filter
- Both call addFolderFilterConditions()
- Both add duration filter conditions

The duration filter building is particularly duplicated (lines 98-107 and 155-164).

**Suggested approach:**
Extract a shared `buildFilterConditions()` function that returns `{ conditions, params, audioJoin }`, then have both searchAssets and countSearchResults call it.


## CLAUDE.md Updates
When implementing this, update `src/lib/database/CLAUDE.md` if the shared filter builder becomes part of the public API (unlikely — internal refactor only).

## Summary of Changes

Extracted `buildFilterConditions()` from the duplicated filter-building logic in `searchAssets()` and `countSearchResults()`. The shared function returns `{ conditions, params, audioJoin }` — `searchAssets` uses conditions/params (ASSET_JOINS already covers the audio join), while `countSearchResults` uses all three.
