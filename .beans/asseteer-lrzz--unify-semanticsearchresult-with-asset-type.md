---
# asseteer-lrzz
title: Unify SemanticSearchResult with Asset type
status: todo
type: task
priority: normal
created_at: 2026-03-20T11:43:41Z
updated_at: 2026-03-20T11:43:41Z
parent: asseteer-38rb
---

`SemanticSearchResult` in `src/lib/database/queries.ts` (lines 607-627) duplicates most fields from the `Asset` type in `src/lib/types/index.ts` (lines 1-25), just adding `similarity` and omitting `width`/`height`.

This causes awkward mapping in the library page (`src/routes/(app)/library/+page.svelte` lines 83-94) where semantic results need `width: null, height: null` added to be compatible with Asset[].

**Suggested approach:**
Make SemanticSearchResult extend Asset (or use `Asset & { similarity: number }`). The Rust backend already returns null for width/height on audio assets, so the types should align naturally.
