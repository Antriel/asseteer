---
# asseteer-zwdl
title: Removing a folder is slow, lacks feedback, and doesn't shrink the DB
status: completed
type: bug
priority: normal
created_at: 2026-03-24T11:44:11Z
updated_at: 2026-03-25T11:34:14Z
---

When removing a folder from the Folders UI:

1. **No user feedback** — the remove operation runs silently with no progress indicator or toast while DB work is happening
2. **Slow** — the DB operations take a long time; the deletion/cleanup patterns need optimization
3. **DB file stays large** — after removal, the SQLite database file does not shrink (no VACUUM or similar is run after deleting assets)


## Plan

- [x] **Backend**: Rewrite `remove_folder` to delete in batches with progress events, then checkpoint + VACUUM
- [x] **Frontend**: Add removing state with spinner/progress on the folder card, disable actions during removal

## Implementation Notes

- Delete assets in batches (e.g. 5000 at a time) with progress events
- Use `AppHandle` to emit `folder-remove-progress` events
- Run VACUUM after deletion to reclaim disk space
- Checkpoint WAL after deletion
- Frontend: track `removingId` state, show spinner on folder card, disable buttons


## Summary of Changes

**Backend** (`src-tauri/src/commands/folders.rs`):
- Batch-delete assets in chunks of 5,000 instead of one massive CASCADE transaction
- Emit `folder-remove-progress` events with phase/deleted/total for live progress
- WAL checkpoint after deletion to flush pages and truncate WAL
- No VACUUM — it rewrites the entire DB into WAL, too slow and leaves huge WAL behind

**Frontend** (`src/routes/(app)/folders/+page.svelte`):
- Spinner + live progress text on the folder card being removed
- All folder action buttons disabled during removal
- Proper event listener cleanup
