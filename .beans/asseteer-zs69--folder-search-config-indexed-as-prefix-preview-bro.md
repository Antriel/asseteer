---
# asseteer-zs69
title: 'Folder search config: ''Indexed as'' prefix preview broken, UX needs rethink'
status: completed
type: bug
priority: normal
created_at: 2026-03-19T11:40:27Z
updated_at: 2026-03-20T08:53:44Z
parent: asseteer-kvnt
---

In folder view, the 'Indexed as:' preview doesn't update when selecting a different PREFIX. Also, some packs (e.g. everything-in-zips) show no PREFIX option at all. The current single-prefix + skip-count model may not be expressive enough. Possible alternative: a tree view where user expands directories and checks/unchecks starting points (or individual path segments) — depends on what the backend can support.


## Resolution

Replaced with tree-based UI in asseteer-wclt. The prefix + skip_depth model was replaced with a per-segment exclude model (folder_search_excludes table). Users now see their actual directory tree and check/uncheck segments. Works for both filesystem and ZIP-internal paths.
