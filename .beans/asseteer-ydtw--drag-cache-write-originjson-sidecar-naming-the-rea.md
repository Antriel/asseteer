---
# asseteer-ydtw
title: 'Drag cache: write .origin.json sidecar naming the real source'
status: completed
type: feature
priority: high
created_at: 2026-10-13T14:57:46Z
updated_at: 2026-10-13T15:13:10Z
---

When a drag (or Ctrl+C copy) has to materialize an asset into the drag cache, write an
`<filename>.origin.json` sidecar next to the cached file naming where it really came from. A
receiving program that cares (Scry's import, scry-app-k8aq) reads it; everything else ignores it.

## Why

Scry is getting a drag/drop import that copies a sound into the game project and records where it
came from in a provenance ledger, so that years later "which pack was this from" and the ship-time
license credits can be answered. A plain local file needs nothing: the dropped path *is* the
origin. But a ZIP entry (incl. nested zips) or a network-drive file arrives as
`<appdata>/drag-cache/<id>_<mtime>/<filename>`, which says nothing. Asseteer is the only party that
knows the real location, and it already has it in `format_asset_display_path`. A sidecar keeps Scry
from ever having to understand zips or asseteer's database.

## Contract (Scry depends on exactly this)

- **Where:** beside the cached file, same directory: `<cache>/<id>_<mtime>/<filename>.origin.json`.
  Only ever inside the drag cache. Never write anything next to a file that is dragged in place
  (local, non-network): that is the user's library, and the dropped path is already the origin.
- **Content:** UTF-8 JSON object, `{ "path": "<origin>" }`. Receivers ignore unknown fields, so it
  can grow later.
- **`path` value:** the same string "Copy path" (`copyAssetPaths`) puts on the clipboard for that
  asset: the display path with separators normalized to the OS separator, e.g.
  `D:\Packs\Retro.zip\sub.zip\Sounds\coin.wav`. Identical on purpose: a path pasted by hand into
  Scry and one read from a sidecar must produce the same record. For a network-drive file it is the
  original UNC / mapped path.
- **Ordering:** the sidecar must exist whenever the cached file does. Write it before the
  `.partial` → final rename (write the sidecar itself via temp + rename too, or at least before the
  main rename), so a receiver never sees the file without it.
- **Backfill:** a cache entry created before this change has the file but no sidecar. On reuse
  (`target.is_file()` branch) write the sidecar if it is missing.
- The drag payload itself stays `DragItem::Files([cached file])`. The sidecar is not dragged.

## Todo

- [x] Write the sidecar in `ensure_local_file` (`src-tauri/src/commands/external.rs`) for both the
      zip-extract and network-copy branches, per the ordering rule above
- [x] Backfill a missing sidecar on cache reuse
- [x] Shared helper for the "Copy path" string on the Rust side (or move the separator
      normalization there) so frontend copy and sidecar cannot drift
- [x] Tests: zip entry gets a sidecar with the display path; nested zip path; local in-place file
      writes nothing (extend `local_file_is_used_in_place`); reuse backfills; prune still removes
      a stale dir containing a sidecar
- [x] Note the sidecar in the module doc comment of `external.rs` as a public contract

## Summary of Changes

- `utils::asset_origin_path` is the one definition of the Copy path string (display path, separators to `MAIN_SEPARATOR`). The new `get_asset_origin_paths` command serves it to `copyAssetPaths`, so the frontend no longer builds the string itself and cannot drift from the sidecar.
- `ensure_local_file` writes `<filename>.origin.json` (`{"path": ...}`) via `.partial` + rename *before* the cached file is written, for both the zip-extract and network-copy branches. Files used in place get nothing.
- On cache reuse the sidecar is rewritten when it is missing **or its content differs**. That covers the backfill, and also a source folder that was renamed (same id and mtime, so the cache entry is reused).
- Contract documented in the `external.rs` module doc and `src-tauri/CLAUDE.md`.
- Tests: Rust unit tests (zip entry, nested zip chain, in-place writes nothing, reuse backfill + refresh, prune with a sidecar). New e2e `tests/e2e/external.spec.mjs` checks against the real app that Copy Path on the nested-zip entry equals the sidecar Ctrl+C produces, and that a plain file gets no sidecar.
