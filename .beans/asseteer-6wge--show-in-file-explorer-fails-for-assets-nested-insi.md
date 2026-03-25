---
# asseteer-6wge
title: Show in file explorer fails for assets nested inside a zip-within-a-zip
status: todo
type: bug
created_at: 2026-03-25T12:30:05Z
updated_at: 2026-03-25T12:30:05Z
---

'Show in file explorer' works correctly for assets inside a top-level zip file on Windows, but fails when the asset is inside a nested zip (a zip within a zip).

Fix: when the asset path contains nested zips, open the file explorer to the deepest zip file that is a real filesystem path (i.e. the first/outermost nested zip), rather than trying to navigate inside it further. Do not attempt to open into inner zips.
