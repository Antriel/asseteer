---
# asseteer-uh17
title: Removing folder errors with FOREIGN KEY constraint failed
status: completed
type: bug
priority: normal
created_at: 2026-03-20T06:38:47Z
updated_at: 2026-03-20T07:17:57Z
parent: asseteer-kvnt
---

Removing a folder in folder management errors out with:

`Failed to remove folder: Failed to remove folder: error returned from database: (code: 787) FOREIGN KEY constraint failed`

Before fixing, check if we have tests set up for this area and add tests first if so.


## Summary of Changes

- Fixed `CREATE_SCAN_SESSIONS_TABLE` schema to add `ON DELETE CASCADE` on `source_folder_id` (for new databases)
- Added explicit `DELETE FROM scan_sessions WHERE source_folder_id = ?` before deleting the folder in `remove_folder` command (handles existing databases without needing a migration)
