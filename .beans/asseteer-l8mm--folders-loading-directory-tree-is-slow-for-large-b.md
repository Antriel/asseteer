---
# asseteer-l8mm
title: 'Folders: ''Loading directory tree...'' is slow for large bundles'
status: todo
type: bug
created_at: 2026-03-23T15:43:13Z
updated_at: 2026-03-23T15:43:13Z
---

In the Folders page, the search indexing config step 'Loading directory tree...' takes a very long time for large imported bundles. This should query the DB (which is instant) rather than walking the actual filesystem. Needs investigation to confirm which path is used and fix if filesystem-based.
