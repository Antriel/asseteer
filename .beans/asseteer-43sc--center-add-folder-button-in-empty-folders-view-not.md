---
# asseteer-43sc
title: Center 'Add Folder' button in empty folders view not disabled during import
status: todo
type: bug
created_at: 2026-03-25T12:12:21Z
updated_at: 2026-03-25T12:12:21Z
---

When the folders view has no folders yet, a centered 'Add Folder' button is shown. Unlike the regular add folder button, this one is not disabled when an import is in progress.

Clicking it during an import causes the UI card/progress for the ongoing import to disappear (the import itself continues in the background and finishes successfully, but there is no visible progress). Navigating away and back to the Folders page then shows the folder as imported with 0 assets, which is incorrect.

Two things to fix:
1. Disable the center 'Add Folder' button while an import/scan is in progress (same as the regular button).
2. After navigating away and back, the folder that was importing in the background shows 0 assets — investigate why the asset count is wrong after a backgrounded import completes.
