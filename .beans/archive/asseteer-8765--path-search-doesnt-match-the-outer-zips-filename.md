---
# asseteer-8765
title: Path search doesn't match the outer zip's filename
status: completed
type: bug
priority: normal
created_at: 2026-09-25T09:53:20Z
updated_at: 2026-09-25T12:25:53Z
---


Searching with scope **Path** for `retro` finds nothing, although the three `retro_*` fixture sounds live in `Packs/Retro Pack.zip/...` (Anywhere/Name find them via the filename).

## Cause
`compute_searchable_path` (`src-tauri/src/commands/scan.rs`) builds `searchable_path` from the `rel_path` segments plus the *directory part of `zip_entry`* — the outer `zip_file` name is never pushed. Inconsistent with nested zips: their name is part of `zip_entry` (`Extras/bonus.zip/retro_powerup.wav`) so `bonus.zip` *is* indexed.

## Fix
- [x] Push the zip file's name (probably without `.zip`, split on separators like other segments) into `searchable_path`, honouring `excludes` the same way
- [x] Existing libraries: re-index `searchable_path` for zip assets (migration or on-startup recompute — `folders.rs` already has a recompute loop for excludes)
- [x] Rust unit test for `compute_searchable_path` with a zip asset
- [x] e2e: Path-scoped search for `retro` returns the 3 zip sounds (fixture already has `Packs/Retro Pack.zip`)

## Summary of Changes

- `compute_searchable_path` now pushes the outer zip name after the `rel_path` segments. Zip names, including nested ones from `zip_entry`, are indexed without `.zip`, so a Path search for `zip` no longer matches every archived asset. The outer zip is excludable as a filesystem entity: exclude key `(None, "{rel_path}/{zip_file}")`.
- Search-indexing panel (`SearchConfigPanel.svelte`): zip archive nodes now have a checkbox, and their node path is the cumulative filesystem path.
- `reindex_searchable_paths(pool, folder_id, zip_only)` in `folders.rs` is shared by `update_search_excludes` and a new one-time data migration (`migrate_data` in `database/init.rs`, gated on `PRAGMA user_version`, now 1). It only rewrites rows whose value changed.
- Tests: 3 Rust unit tests for `compute_searchable_path`, a migration test, and an e2e test for Path-scoped `retro` (3), `bonus` (1) and `zip` (0). A scratch harness run checked excluding the zip in the panel: Path `retro` gives 0, then 3 after restoring. Both themes screenshotted.

### Follow-up: migration off the startup path

The migration took about 3 minutes on a 1.4M-asset library. It ran inside `initialize_db` before the window existed, so the app looked hung. Changes:
- It moved to `database/migrate.rs` and now runs on a background task once the window is up. `pending()` stamps an empty library as current, so a fresh install never sees the dialog.
- `reindex_searchable_paths` commits every 5000 rows and reports progress. Each step stamps `user_version` when it finishes.
- `DbMigrationDialog.svelte` in the root layout polls `get_db_migration` and blocks the UI with a progress bar. On failure it shows the error with a Continue button, and the migration runs again on the next start.
- Checked in the real app: 200k fake zip assets at `user_version` 0, then a relaunch. The dialog was up in 0.6 s, showed live progress, and closed after about 20 s with no console errors. Screenshotted in both themes.
