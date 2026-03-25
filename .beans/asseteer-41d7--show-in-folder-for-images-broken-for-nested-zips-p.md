---
# asseteer-41d7
title: 'Show in Folder for images broken for nested ZIPs: path display wrong and tree doesn''t expand fully'
status: todo
type: bug
created_at: 2026-03-25T12:48:53Z
updated_at: 2026-03-25T12:48:53Z
blocking:
    - asseteer-cz2f
---

The 'Show in Folder' button on image assets (which should open the folder pane and expand/highlight the file in the tree) appears to be broken, at least for nested ZIPs:

- No errors are shown, but the tree doesn't expand fully to the asset's location.
- The folder filter is applied, but the path shown is wrong — it displays something like `something.zip/folder` without the full filesystem path leading to the zip. Likely related to the same zip path truncation issue as asseteer-cz2f.
- The tree opens/expands slowly. Investigate whether the optimisations from asseteer-l8mm (which moved zip tree loading to a single Rust command) apply to this code path too, or if it already shares the same path and is already benefiting.
