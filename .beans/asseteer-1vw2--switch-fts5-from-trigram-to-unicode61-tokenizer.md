---
# asseteer-1vw2
title: Improve text search quality and fix tokenizer limitations
status: todo
type: task
priority: normal
created_at: 2026-03-17T08:44:21Z
updated_at: 2026-03-19T06:28:30Z
parent: asseteer-i459
blocked_by:
    - asseteer-wxak
---

Rework full-text search to fix tokenizer limitations, support short patterns, and give users control over which path segments are searchable.

Depends on asseteer-wxak which now includes the full schema (source_folders + folder_id/rel_path + folder_search_config).

## Current problems

1. **Can't search for short patterns** — trigram tokenizer requires at least 3 characters. Searching for `_11` or `02` finds nothing.
2. **No special-character awareness** — underscores, hyphens, and dots in filenames like `dark_forest_ambience_01.wav` aren't treated as word separators, so word-level matching is inconsistent.
3. **Appended `*` wildcard** — the frontend appends `*` for prefix matching, but with trigram this is meaningless since trigram already does substring matching.
4. **Duplicate words are no-ops** — searching `zip zip` is the same as `zip` because FTS5 implicit AND deduplicates. No way to require two distinct occurrences (e.g. nested zip inside a zip).
5. **Path noise** — every search hits all path segments equally. A folder like `D:\Assets\HumbleBundle2024\GameAudio\Nature\Ambient\forest_01.wav` means searching `GameAudio` pollutes results even though it's organizational noise, not descriptive content.

## Decision: Dual FTS tables

Use **two FTS tables** — trigram for substring matching, unicode61 for word/short-pattern matching. Query both and merge results.

### Trigram table (substring search)
Preserves current behavior: `fire` finds `fireplace`, `bubble` finds `bubbles`, `rest` finds `forest`.

```sql
CREATE VIRTUAL TABLE assets_fts_sub USING fts5(
    filename, searchable_path,
    tokenize='trigram'
);
```

### Unicode61 table (word search)
Handles short patterns and word-boundary matching. `01` finds `texture_01.png`. Phrase query `"zip zip"` requires two adjacent occurrences.

```sql
CREATE VIRTUAL TABLE assets_fts_word USING fts5(
    filename, searchable_path,
    tokenize="unicode61 separators '_-.'"
);
```

### Query strategy
- Search both tables, UNION the rowid results, deduplicate
- For multi-word queries: the word table handles AND/phrase semantics correctly
- For short patterns (< 3 chars): only query the word table (trigram can't help)
- For longer queries: query both, trigram catches substring hits the word table misses
- Drop the meaningless `*` append for trigram; use `*` prefix expansion only on the word table where it's meaningful

### Write cost
Both tables are populated by triggers on the assets table. Writes only happen during scans (not during normal browsing/searching), so the double write cost is negligible. The FTS tables are small text — the overhead is trivial compared to audio/image processing.

## Per-folder search depth configuration

### The problem
Asset libraries have organizational path structure that varies per source:
```
D:\Assets\HumbleBundle2024\GameAudio\Nature\Ambient\forest_01.wav
          ^^^^^^^^^^^^^^^^ ^^^^^^^^^ ^^^^^^ ^^^^^^^
          bundle name      pack name  useful  useful
          (noise)          (noise)
```

The user knows which levels are noise. We can't guess. Different folders have different structures.

### Solution: `folder_search_config` table

```sql
CREATE TABLE folder_search_config (
    id INTEGER PRIMARY KEY,
    source_folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    subfolder_prefix TEXT NOT NULL DEFAULT '',  -- e.g. "HumbleBundle2024/GameAudio" or '' for folder root
    skip_depth INTEGER NOT NULL DEFAULT 0,     -- skip first N segments of rel_path below this prefix
    UNIQUE(source_folder_id, subfolder_prefix)
);
```

**Resolution logic**: For a given asset's `rel_path`, find the longest matching `subfolder_prefix` in the config table, apply its `skip_depth`. Fall back to source folder's default (0 = index everything).

**Example**:
- Source folder: `D:\Assets`
- Config: `subfolder_prefix="HumbleBundle2024"`, `skip_depth=1` (skips "GameAudio")
- Asset rel_path: `HumbleBundle2024\GameAudio\Nature\Ambient`
- Indexed `searchable_path`: `Nature Ambient` (skipped "HumbleBundle2024" via prefix match, then skipped 1 more level "GameAudio")

Note: the prefix itself is always excluded from search (it's structural), and `skip_depth` controls how many *additional* levels below the prefix to skip.

### Dynamic re-indexing on config change

Changing search depth is a metadata operation, not a rescan. When the user modifies depth config:

1. Update `folder_search_config` row
2. Query all assets matching `source_folder_id` + `rel_path LIKE 'prefix%'`
3. Recompute `searchable_path` for each by stripping configured segments
4. Bulk `DELETE + INSERT` on both FTS tables for affected rowids
5. This is O(affected assets), not O(all assets) — fast even for large libraries

### UI in folder management
In the folder management page (asseteer-wxak), each source folder expands to show its subfolder tree. Each node has a "search depth" control:
- Visual indicator showing which segments are indexed vs skipped
- Changing it triggers a re-index with a progress indicator
- Reasonable defaults: skip 0 levels (index everything) until user configures otherwise

## FTS columns (post schema from wxak)

Once asseteer-wxak lands (`folder_id + rel_path + zip_file + zip_entry`), the FTS triggers compute:
- `filename` — bare filename from `assets.filename` (always fully indexed)
- `searchable_path` — composed of two parts:
  1. **Filesystem path**: `assets.rel_path` minus skipped segments per `folder_search_config`
  2. **ZIP-internal path**: directory portion of `assets.zip_entry` (if present) — always included, not affected by search depth config (ZIP-internal structure is content, not organizational noise)

Both parts are space-separated for tokenization. Path separators replaced with spaces before indexing.

## Search UI

### Column targeting (search bar toggle)
Simple toggle on the search bar:
- **Anywhere** (default) — searches both filename and searchable_path columns
- **Filename only** — `{filename}: query` in FTS5 syntax
- **Path only** — `{searchable_path}: query` in FTS5 syntax

### Multi-word queries
- `forest ambient` → AND: both words must appear somewhere
- `"forest ambient"` → PHRASE: exact word sequence (useful for `"zip zip"` — requires two adjacent occurrences in the word table)
- Short tokens (< 3 chars) routed to word table only

### Integration with folder tree
The folder tree sidebar already filters by directory. Combined with configurable search depth, this gives users two complementary narrowing mechanisms:
- **Folder tree**: "I want to look inside this specific pack" (navigational)
- **Text search**: "Find assets matching this description" (content)
- **Search depth config**: "These path levels are noise, don't pollute my results" (one-time setup)

## Implementation tasks

- [ ] Create `folder_search_config` table (migration, after wxak lands)
- [ ] Create `assets_fts_sub` (trigram) table and triggers
- [ ] Create `assets_fts_word` (unicode61) table and triggers
- [ ] Implement `searchable_path` computation in triggers (respecting folder_search_config)
- [ ] Implement re-index logic for dynamic search depth changes
- [ ] Update frontend query builder to search both FTS tables and merge results
- [ ] Route short patterns (< 3 chars) to word table only
- [ ] Add column targeting toggle to search bar UI
- [ ] Add search depth config UI in folder management page
- [ ] Drop old `assets_fts` table and triggers
- [ ] Remove meaningless `*` wildcard append from frontend

## Dependencies

- **Blocked by** asseteer-wxak (source_folders + folder_id/rel_path schema + folder_search_config table)
- FTS triggers need access to `folder_search_config` to compute `searchable_path`
