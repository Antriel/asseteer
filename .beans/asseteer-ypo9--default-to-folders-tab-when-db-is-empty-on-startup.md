---
# asseteer-ypo9
title: Default to Folders tab when DB is empty on startup
status: completed
type: feature
priority: normal
created_at: 2026-03-25T12:04:31Z
updated_at: 2026-03-25T15:33:13Z
---

When the app starts with an empty database (no folders added yet), it should automatically navigate to the Folders tab instead of showing an empty library.

## Summary of Changes

Modified `src/routes/+page.svelte` to query `source_folders` count on startup. If no active folders exist, redirects to `/folders`; otherwise redirects to `/library` as before.
