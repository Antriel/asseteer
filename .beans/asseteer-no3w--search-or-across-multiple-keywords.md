---
# asseteer-no3w
title: 'Search: OR across multiple keywords'
status: todo
type: feature
priority: high
created_at: 2026-09-25T08:09:23Z
updated_at: 2026-09-25T08:09:23Z
---

User request: "search on multiple keywords" — meaning **OR** (find either), not AND.

## Current behavior (verified 2026-09-25 against scratch SQLite)
`buildFtsCondition` in `src/lib/database/queries.ts` passes raw input as an FTS5 query to both `assets_fts_sub` (trigram) and `assets_fts_word` (unicode61), UNIONed.
- Space-separated words already AND (FTS5 implicit AND): `gun shot`, `gun weapons` (cross-column) work.
- Punctuation is parsed as FTS syntax: `sci-fi` → "no such column: fi"; commas, quotes, parens also break.
- Errors are swallowed to console (`assets.svelte.ts` ~line 120) — user just sees stale/empty results.
- The `< 3 chars` branch is decided on the whole query, not per term (trigram matches nothing for sub-3-char terms).

## Proposed design
- **Comma = OR, space = AND**: `gun shot, explosion` → (gun AND shot) OR explosion. Commas are the natural list separator and the exact thing that breaks today.
- Tokenize input ourselves; quote every term (escape `"`), so no user text is ever FTS syntax.
- Per term: <3 chars → word table with prefix `*`; else trigram substring UNION word prefix. Build per-term rowid sets and combine with INTERSECT (AND) / UNION (OR), or build one FTS expression per table if equivalent.
- Keep column targeting (`filename:` / `searchable_path:`).
- Surface query errors in UI (toast or inline) instead of console-only.
- Alternative/addition: an Any/All toggle next to the search box. Start with commas; add toggle if discoverability is a problem.

## Open: ranking
With OR, rows matching more groups should rank first. Two FTS tables make bm25 awkward — consider counting matched groups (sum of per-group membership) as the sort key. Check current ORDER BY first.

## Tests
`buildFtsCondition` is pure — write vitest cases first (`src/**/*.test.ts`, harness session added vitest). Ideally also an integration test against real FTS5 tables (Rust side has `concurrent_tests.rs` search_test.db pattern). Verify via harness with the fixture library.

- [ ] vitest cases for tokenizer/query builder
- [ ] implement tokenizer + per-term query
- [ ] OR groups via comma
- [ ] ranking by matched groups
- [ ] error surfacing
- [ ] search box hint/placeholder documenting syntax
- [ ] verify via harness
