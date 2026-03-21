---
# asseteer-xfu1
title: Deduplicate query filter building in queries.ts
status: todo
type: task
priority: normal
created_at: 2026-03-20T11:43:33Z
updated_at: 2026-03-20T11:48:58Z
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
