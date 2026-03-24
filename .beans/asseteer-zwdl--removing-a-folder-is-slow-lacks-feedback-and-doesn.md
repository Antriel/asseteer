---
# asseteer-zwdl
title: Removing a folder is slow, lacks feedback, and doesn't shrink the DB
status: todo
type: bug
created_at: 2026-03-24T11:44:11Z
updated_at: 2026-03-24T11:44:11Z
---

When removing a folder from the Folders UI:

1. **No user feedback** — the remove operation runs silently with no progress indicator or toast while DB work is happening
2. **Slow** — the DB operations take a long time; the deletion/cleanup patterns need optimization
3. **DB file stays large** — after removal, the SQLite database file does not shrink (no VACUUM or similar is run after deleting assets)
