---
# asseteer-8765
title: Path search doesn't match the outer zip's filename
status: todo
type: bug
created_at: 2026-09-25T09:53:20Z
updated_at: 2026-09-25T09:53:20Z
---

Searching with scope **Path** for `retro` finds nothing, although the three `retro_*` fixture sounds live in `Packs/Retro Pack.zip/...` (Anywhere/Name find them via the filename).

## Cause
`compute_searchable_path` (`src-tauri/src/commands/scan.rs`) builds `searchable_path` from the `rel_path` segments plus the *directory part of `zip_entry`* — the outer `zip_file` name is never pushed. Inconsistent with nested zips: their name is part of `zip_entry` (`Extras/bonus.zip/retro_powerup.wav`) so `bonus.zip` *is* indexed.

## Fix
- [ ] Push the zip file's name (probably without `.zip`, split on separators like other segments) into `searchable_path`, honouring `excludes` the same way
- [ ] Existing libraries: re-index `searchable_path` for zip assets (migration or on-startup recompute — `folders.rs` already has a recompute loop for excludes)
- [ ] Rust unit test for `compute_searchable_path` with a zip asset
- [ ] e2e: Path-scoped search for `retro` returns the 3 zip sounds (fixture already has `Packs/Retro Pack.zip`)
