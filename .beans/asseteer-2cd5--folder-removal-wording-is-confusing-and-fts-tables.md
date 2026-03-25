---
# asseteer-2cd5
title: Folder removal wording is confusing and FTS tables may not be cleaned up
status: todo
type: bug
priority: normal
created_at: 2026-03-25T12:04:31Z
updated_at: 2026-03-25T12:04:35Z
---

Two issues with folder removal:

1. The progress message says "removing assets" which is scary/confusing — we are only removing DB entries, not actual files. Wording should be clarified.

2. We may be leaving orphaned data in FTS tables: `assets_fts_sub_data`, `assets_fts_sub_idx`, `assets_fts_word_data`, `assets_fts_word_idx`. This is likely because there is no DELETE CASCADE on those tables. Note: FTS indexing was moved to a post-import stage (not during import) to speed up importing — any fix must preserve that behaviour.
