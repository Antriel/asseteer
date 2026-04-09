---
# asseteer-9f2b
title: 'Favorites: tag assets and view favorites list'
status: draft
type: feature
created_at: 2026-04-03T08:21:15Z
updated_at: 2026-04-03T08:21:15Z
---

Allow users to tag sounds (and eventually images) as favorites while browsing, and provide a dedicated favorites view to return to them easily.

## Goals
- Quick way to mark/unmark a sound as favorite while searching/browsing
- Dedicated favorites list/view to see all favorited sounds
- Persistent across sessions (stored in DB)
- Should generalize to images too eventually, but audio is the priority

## Scope
- Favorite toggle button on audio assets (in list row and context menu)
- Favorites tab or filter in the library view
- DB schema: favorites table or a flag on assets
- Backend: add/remove favorite commands
- Frontend: favorites state, filter, and list view
