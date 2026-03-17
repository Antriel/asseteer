---
# asseteer-1vw2
title: Switch FTS5 from trigram to unicode61 tokenizer
status: todo
type: task
priority: high
created_at: 2026-03-17T08:44:21Z
updated_at: 2026-03-17T08:55:53Z
parent: asseteer-i459
---

The `trigram` tokenizer creates an index entry for every 3-character subsequence of every string. A filename like `forest_ambience_01.wav` generates ~20 trigrams; a full path generates ~35 more. With 100K assets, the FTS tables can be **3-5x larger than the assets table itself**.

## Current

```sql
CREATE VIRTUAL TABLE assets_fts USING fts5(
    filename, path_segments,
    tokenize='trigram'
);
```

## Proposed

Switch to `unicode61` (word-based tokenizer). Current `*` suffix for prefix matching works with unicode61 too. If substring matching is needed (e.g. "rest" → "forest"), consider:
- `unicode61` on filename only (drop path_segments from FTS entirely — it's rarely searched by substring)
- `LIKE '%rest%'` fallback for the rare substring case

## Impact
- **Filesize:** Largest single win — could reduce DB size by 30-50% depending on asset count
- **Write performance:** Fewer index entries per INSERT = faster scanning
- **Read performance:** Smaller index = faster FTS queries


## Future consideration: directory-filtered search

The advanced search feature will need separate searching of filenames vs directory paths. FTS5 supports column-specific matching (e.g. `{filename}: forest` or `{dir_segments}: textures`).

**Do NOT drop `path_segments` from FTS.** Instead, rename it to `dir_segments` and index only the directory path components (excluding filename, which is already in column 1). This enables:
- Search filenames only: `{filename}: forest*`
- Search directories only: `{dir_segments}: textures*`
- Search both (default): `forest*`

With `unicode61`, directory words become directly matchable without trigram bloat.
