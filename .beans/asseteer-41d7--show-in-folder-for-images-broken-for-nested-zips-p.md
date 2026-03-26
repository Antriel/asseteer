---
# asseteer-41d7
title: 'Show in Folder for images broken for nested ZIPs: path display wrong and tree doesn''t expand fully'
status: completed
type: bug
priority: normal
created_at: 2026-03-25T12:48:53Z
updated_at: 2026-03-26T09:00:02Z
blocking:
    - asseteer-cz2f
---

The 'Show in Folder' button on image assets (which should open the folder pane and expand/highlight the file in the tree) appears to be broken, at least for nested ZIPs:

- No errors are shown, but the tree doesn't expand fully to the asset's location.
- The folder filter is applied, but the path shown is wrong — it displays something like `something.zip/folder` without the full filesystem path leading to the zip. Likely related to the same zip path truncation issue as asseteer-cz2f.
- The tree opens/expands slowly. Investigate whether the optimisations from asseteer-l8mm (which moved zip tree loading to a single Rust command) apply to this code path too, or if it already shares the same path and is already benefiting.

## Summary of Changes

**Path display bug (confirmed and fixed):**
- `Toolbar.svelte` `folderDisplayName`: the zip branch was doing `.split('/').pop()` on `zipPrefix`, which discards all intermediate nested-zip levels (e.g. `inner.zip/folder/` → only `folder`). Also didn't include `relPath` context at all.
- Fixed to build the full path from `relPath` parts + `zipFile` + all `zipPrefix` parts joined with ` / `. Example: `Packs / archive.zip / inner.zip / folder`.

**Tree expansion (investigated — no code bug found):**
- `navigateToAsset` key generation is consistent with `getZipDirectoryChildren` node keys at all nesting levels. The sequential `await loadChildren` calls ensure DOM state is set before the `requestAnimationFrame` scroll.
- The asseteer-l8mm optimisation (N+1 → single Rust command) was specific to `SearchConfigPanel.svelte`. The library folder tree uses `getZipDirectoryChildren` which makes one DB query per expanded level — fine for the 3-4 levels typical of nested zips.
