---
# asseteer-6wge
title: Show in file explorer fails for assets nested inside a zip-within-a-zip
status: completed
type: bug
priority: normal
created_at: 2026-03-25T12:30:05Z
updated_at: 2026-03-26T09:09:31Z
---

'Show in file explorer' works correctly for assets inside a top-level zip file on Windows, but fails when the asset is inside a nested zip (a zip within a zip).

Fix: when the asset path contains nested zips, open the file explorer to the deepest zip file that is a real filesystem path (i.e. the first/outermost nested zip), rather than trying to navigate inside it further. Do not attempt to open into inner zips.

## Summary of Changes

- **`assetActions.ts`** `openDirectory`: for zip assets, now opens the outermost zip file path (e.g. `folder\rel\outer.zip`) instead of just the containing folder. Handles nested zips by taking only the first `/`-separated component of `zip_file`.
- **`ImageLightbox.svelte`**: removed duplicate `openInExplorer` function; now calls `openDirectory(asset)` from `assetActions.ts`.
- **`DirectoryTree.svelte`**: fixed zip case to open the zip file path (not just its parent directory); handles nested zip paths the same way.
