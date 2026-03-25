---
# asseteer-cz2f
title: Current file path display truncates inner zip path and doesn't handle nested zips
status: todo
type: bug
created_at: 2026-03-25T12:30:05Z
updated_at: 2026-03-25T12:30:05Z
---

When showing the currently-processing file, the displayed path only goes up to the zip file and the entry inside it, but ignores any inner zip path (e.g. for a file nested inside a zip-within-a-zip). Nested zips are not handled at all.

Need to:
1. Show the full path including inner zip hierarchy when displaying the current file being processed.
2. Check all other places in the UI where asset paths are displayed and unify them to show the actual full path consistently.
3. Handle arbitrarily nested zips in the path display logic.
