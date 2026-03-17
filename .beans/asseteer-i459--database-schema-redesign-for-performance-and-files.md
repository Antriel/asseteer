---
# asseteer-i459
title: Database schema redesign for performance and filesize
status: todo
type: epic
priority: normal
created_at: 2026-03-17T08:43:21Z
updated_at: 2026-03-17T08:56:01Z
---

Comprehensive DB redesign to improve query performance and reduce filesize. Covers FTS tokenizer, thumbnail separation, path normalization, embedding search, and index optimization. Planned alongside source_folders (asseteer-wxak).


## Future consideration: directory-filtered search

A planned feature will allow filtering search to specific directories and searching directory names separately from filenames. The children of this epic have been annotated with notes on how to support this:
- **asseteer-1vw2**: Keep two FTS columns (`filename` + `dir_segments`), don't drop path from FTS
- **asseteer-x20r**: Use covering index `(folder_id, id)` for folder+FTS intersection queries
- **asseteer-zmc8**: `folder_id` + `rel_path` naturally enables folder-scoped and subdirectory-scoped search
