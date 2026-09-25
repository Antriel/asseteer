---
# asseteer-6nzo
title: 'Semantic search: OR across comma-separated alternatives'
status: completed
type: feature
priority: normal
created_at: 2026-09-25T10:50:57Z
updated_at: 2026-09-25T11:06:18Z
---

Follow-up to asseteer-no3w. In semantic (CLAP) mode, comma-separated alternatives are embedded separately and each sound scores its best match (max cosine). Same comma/chip syntax as text search; quotes keep a descriptive comma literal.

- [x] Rust: command takes a list of queries, max-merges, returns matched alternative index
- [x] cache::search over several query vectors + Rust tests
- [x] frontend: send parsed alternatives, chips in semantic mode (purple)
- [x] semantic-mode wording in the idle hint
- [x] show which alternative a result matched
- [x] verify UI via harness (CLAP stubbed in-page)
- [x] manual test with real CLAP (Peter): works well; keep max-merge (reciprocal rank fusion is the fallback if one alternative ever dominates)

## Progress

- `embedding::best_similarity(queries, emb)` → (index, max cosine); `cache::search` takes `&[Vec<f32>]`, `SimilarityResult.matched_query`. Similarity search passes one vector.
- `search_audio_semantic(queries: Vec<String>, ...)`: embeds alternatives concurrently (JoinSet, the client is `&'static`), with a 64-entry text-embedding cache so committed chips aren't re-embedded per keystroke. Result carries `matched_query`.
- Frontend: `semanticAlternatives()` in searchQuery.ts (one phrase per alternative, quotes stripped, protect commas); `clapState.lastQueries`; chips enabled in semantic mode (purple); AudioList shows the matched alternative as a small purple label when >1; idle legend has semantic wording.
- Verified: unit 38/38, cargo 72/72, e2e 15/15; screenshots of semantic mode in both themes with `clapState.search` stubbed in-page.

## Summary of Changes

Semantic search takes the same comma syntax as text search: each alternative is embedded separately (concurrently, with a 64-entry text-embedding cache) and each sound scores its best (max cosine) match; results carry `matched_query`, shown as a small purple label when there are several alternatives. Chips work in semantic mode (purple); the idle legend has semantic wording. Also: clearing the search (× or erasing the text) no longer turns semantic mode off — `clapState.clearSearch()` leaves the mode alone and cancels an in-flight search; only the Semantic toggle switches it. `clearSearchText` also cancels a pending debounced search.
