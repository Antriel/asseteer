---
# asseteer-2cd5
title: Folder removal wording is confusing and FTS tables may not be cleaned up
status: completed
type: bug
priority: normal
created_at: 2026-03-25T12:04:31Z
updated_at: 2026-03-26T09:55:21Z
---

Two issues with folder removal:

1. The progress message says "removing assets" which is scary/confusing — we are only removing DB entries, not actual files. Wording should be clarified.

2. We may be leaving orphaned data in FTS tables: `assets_fts_sub_data`, `assets_fts_sub_idx`, `assets_fts_word_data`, `assets_fts_word_idx`. This is likely because there is no DELETE CASCADE on those tables. Note: FTS indexing was moved to a post-import stage (not during import) to speed up importing — any fix must preserve that behaviour.


## Plan

- [x] Fix confirm dialog wording to clarify files are not deleted
- [x] Fix progress message wording ("Deleting assets..." → "Removing from library...")
- [x] Verified FTS cleanup: `assets_ad` trigger already handles DELETE → FTS cleanup, shadow tables are managed by SQLite FTS5 automatically — confirmed non-issue

## Summary of Changes

Fixed confusing wording during folder removal:
- Confirm dialog now says "Remove X from the library? N assets will be unindexed. Files on disk are not affected." instead of "Remove X and all N assets? This cannot be undone."
- Progress message changed from "Deleting assets..." to "Removing from library..."

FTS cleanup investigated and confirmed working: the `assets_ad` trigger fires on each batch DELETE and removes corresponding FTS entries. The FTS5 shadow tables (`_data`, `_idx`) are managed internally by SQLite — no orphaned data.


## FTS5 Tombstone Issue (confirmed)

FTS5 DELETE doesn't immediately purge data from shadow tables — it adds tombstone markers. The actual segment data in `_data` and `_idx` persists until an explicit optimize:
```sql
INSERT INTO assets_fts_sub(assets_fts_sub) VALUES('optimize');
INSERT INTO assets_fts_word(assets_fts_word) VALUES('optimize');
```

- [x] Add FTS5 optimize calls after batch asset deletion in `remove_folder`
