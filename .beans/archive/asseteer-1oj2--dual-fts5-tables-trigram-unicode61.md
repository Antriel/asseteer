---
# asseteer-1oj2
title: Dual FTS5 tables (trigram + unicode61)
status: scrapped
type: feature
priority: normal
created_at: 2026-03-19T10:30:35Z
updated_at: 2026-03-19T10:40:24Z
---

Switch from single FTS5 trigram table to dual tables for better short-pattern and word-boundary search

## Summary of Changes

### Backend (Rust)
- **schema.rs**: Added `searchable_path TEXT NOT NULL DEFAULT ''` column to assets table. Replaced single `assets_fts` table with dual FTS5 tables: `assets_fts_sub` (trigram tokenizer for substring matching) and `assets_fts_word` (unicode61 tokenizer with `_-.` separators for word/short-pattern matching). Simplified triggers to copy `filename` and `searchable_path` into both FTS tables.
- **init.rs**: Updated to create both FTS tables.
- **scan.rs**: Added `searchable_path` field to `DiscoveredAsset`. Added `compute_searchable_path()` function that handles prefix stripping and depth skipping from folder search config. Added `load_search_config()` helper. Updated `insert_asset_chunk` and all discovery functions to compute and store `searchable_path`.
- **rescan.rs**: Updated to load search config and pass through discovery. INSERT and UPDATE queries now include `searchable_path`.
- **folders.rs**: Added `update_search_config` command that replaces folder config entries and re-indexes all assets for that folder.
- **lib.rs**: Registered `update_search_config` in invoke_handler.
- **concurrent_tests.rs**: Updated FTS table references from `assets_fts` to `assets_fts_word`.

### Frontend (SvelteKit)
- **queries.ts**: Added `SearchColumn` type and `buildFtsCondition()` helper implementing dual-table query logic (short patterns < 3 chars use word table only; longer patterns UNION both tables). Added `searchColumn` parameter to `searchAssets()` and `countSearchResults()`.
- **assets.svelte.ts**: Added `searchColumn` state field, passed through to query functions.
- **Toolbar.svelte**: Added search column targeting dropdown (Anywhere/Filename/Path) next to search input, hidden during semantic/similarity search modes.

## Reasons for Scrapping
Duplicate of asseteer-1vw2. Work was tracked there instead.
