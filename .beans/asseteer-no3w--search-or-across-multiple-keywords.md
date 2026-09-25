---
# asseteer-no3w
title: 'Search: OR across multiple keywords'
status: completed
type: feature
priority: high
created_at: 2026-09-25T08:09:23Z
updated_at: 2026-09-25T10:41:52Z
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

- [x] vitest cases for tokenizer/query builder
- [x] implement tokenizer + per-term query
- [x] OR groups via comma
- [x] ranking by matched groups
- [x] error surfacing
- [x] search box hint/placeholder documenting syntax
- [x] verify via harness

## Summary of Changes

**Query** — new pure module `src/lib/database/searchQuery.ts`: tokenizer (space = AND, comma = OR, double quotes keep spaces/commas literal), every term quoted so user text is never FTS5 syntax (`sci-fi`, `(1)`, `a:b` all safe). Per term: trigram substring UNION word prefix, word-only under 3 chars (decided per term, not per query). Terms INTERSECT, alternatives UNION, each nested in `SELECT id FROM (...)` so compound precedence can't leak. With 2+ alternatives, `searchAssets` orders by number of alternatives matched (sum of `id IN (alt)`), then filename. Column targeting kept. `queries.ts` uses it via `buildFtsFilter`.

**Tests** — `searchQuery.test.ts` runs the builder against real FTS5 tables (node:sqlite, same tokenizers as schema.rs): AND across columns, OR, mixed short/long terms, ranking, punctuation, diacritics, column targeting, separator-only terms. Added `@types/node` for that (and dropped the now-unused `@ts-expect-error` in vite.config.js). e2e: the `sci-fi` fixme is live (2 results: the file + `Sci-Fi/` path), plus specs for comma chips / click-to-edit / Backspace and the zero-result suggestion.

**Errors** — failed loads now toast (`Search failed: ...`) as well as logging.

**Teaching UI** — no help text in the field; the syntax is taught where it's needed:
- Typing a comma commits the text before it as a chip, with a small `or` between chips and `or…` as the input placeholder. Click a chip to edit it (swaps with the input), × to drop, Backspace in an empty input pulls the last chip back. Chips + input sit in a horizontally scrolling strip (left fade when scrolled, wheel scrolls it). Text search only; semantic/similarity take text as typed.
- Idle empty state: two-line legend, `gun shot` → all of the words, `gun or laser` chips → any of them.
- A several-word search with zero results (incl. `gun or explosion`) shows "No audio match all N words" + a one-click `gun or explosion · 4 audio →` suggestion (count precomputed with the same filters). `assetsState.orSuggestion`.
- Toolbar now mirrors external changes to `assetsState.searchText` (the suggestion, and the empty state's existing "Clear search" which used to leave the field stale).

Harness: `app.search()` clears existing chips first; `searchBox` is found by `aria-label="Search"` (the placeholder changes to `or…`).
