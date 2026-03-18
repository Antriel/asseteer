---
# asseteer-1vw2
title: Improve text search quality and fix tokenizer limitations
status: todo
type: task
priority: normal
created_at: 2026-03-17T08:44:21Z
updated_at: 2026-03-17T08:55:53Z
parent: asseteer-i459
---

The current `trigram` tokenizer has functional limitations that hurt search quality. This bean is about making search work better, not about DB size.

## Current problems

1. **Can't search for short patterns** — trigram tokenizer requires at least 3 characters. Searching for `_11` or `02` finds nothing.
2. **No special-character awareness** — underscores, hyphens, and dots in filenames like `dark_forest_ambience_01.wav` aren't treated as word separators, so word-level matching is inconsistent.
3. **Appended `*` wildcard** — the frontend appends `*` for prefix matching (`searchText.trim() + "*"`), but with trigram this is largely meaningless since trigram already does substring matching.

## Current setup

```sql
CREATE VIRTUAL TABLE assets_fts USING fts5(
    filename, path_segments,
    tokenize='trigram'
);
```

The FTS trigger indexes the full path with separators replaced by spaces into `path_segments`, and the bare filename into `filename`.

## Desired search behaviors

- **Substring match**: `forest` finds `dark_forest_ambience.wav` ✓ (works now)
- **Short patterns**: `_11` or `02` finds `texture_11.png`, `sound_02.wav` ✗ (broken)
- **Numeric suffixes**: `01` finds all files ending in `01` ✗ (broken — too short for trigram)
- **Extension search**: `.wav` or `wav` finds all WAV files (nice to have)
- **Directory filtering**: search within a specific folder (partially working already)

## Options to investigate

### Option A: Custom tokenizer with `unicode61`
Use `unicode61` with `separators` option to treat `_`, `-`, `.` as word boundaries. This gives word-level search with good filename splitting. Loses pure substring matching but gains reliable word matching.

```sql
tokenize="unicode61 separators '_-.'"
```
Pros: Small index, fast, handles `01` and `11` as words. Cons: `rest` won't find `forest` (not a word boundary match).

### Option B: Dual FTS tables
Keep `trigram` for substring search, add a `unicode61` table for word/short-pattern search. Query both and merge results.

Pros: Best of both worlds. Cons: Double write cost, more complex queries.

### Option C: trigram + supplemental LIKE queries
Keep trigram as primary, fall back to `LIKE` for patterns under 3 chars.

Pros: Minimal change. Cons: LIKE on short patterns is a full scan, could be slow.

### Option D: Keep trigram, preprocess search input
Split search input on `_`, `-`, `.` and search for each token. Pad short tokens.

Pros: No schema change. Cons: Hacky, doesn't solve the fundamental 3-char minimum.

## Decision needed

Profile actual search usage patterns and pick the approach that best balances substring matching (which users rely on) with short-pattern support (which is currently broken). Don't sacrifice existing substring search functionality for DB size savings.
