---
# asseteer-3e94
title: Drag audio out of Asseteer into other programs
status: completed
type: feature
priority: normal
created_at: 2026-09-25T08:09:37Z
updated_at: 2026-09-25T11:34:41Z
---

User request: "being able to drag+drop a found sample into another program (like Audacity) saves a bunch of movements."

## Constraints
- Tauri 2 has no built-in drag-out of files. Candidate: CrabNebula `tauri-plugin-drag` (`@crabnebula/tauri-plugin-drag`, native drag on Windows/macOS/Linux). Verify maintenance + compatibility with Tauri 2.11 first; alternatives: `drag` crate directly from a custom command.
- **Assets inside zips** (`zip_entry` non-null, including nested zips): the target needs a real file. Extract to a temp dir (stable name = original filename, so the DAW shows a sensible name) before the drag starts. Drag is initiated on mousedown/dragstart while the button is held, so either extract fast on demand (plugin's startDrag is async — test if it tolerates the delay) or pre-extract on hover/selection. Reuse `zip_cache.rs` machinery. Temp cleanup policy needed (on app exit / LRU).
- Must not conflict with row click/select, keyboard nav, or the context menu. Threshold before drag starts.
- Images: same mechanism should work for the image grid; audio is the priority.
- The CDP harness **cannot** drive OS drag-and-drop — Peter tests manually (Audacity, Explorer, a DAW).

## Plan
1. Spike: plugin wired to one plain (non-zip) audio file row → drag into Audacity/Explorer on Windows.
2. Zip entries: temp extraction path + timing.
3. Polish: drag preview icon, all audio rows, image grid, temp cleanup.

- [x] spike: plugin + plain file
- [x] zip entry extraction for drag
- [x] wire into AudioList rows
- [x] image grid (optional)
- [x] temp cleanup
- [x] manual test by Peter (Audacity + Explorer)

## Decisions (2026-09-25)
- Use `tauri-plugin-drag` 2.1.1 (drag-rs). Scope: drag a single row (audio first, image grid too); multi-select is a follow-up.
- **No pre-extraction on select/hover** (Peter: wastes drive lifetime). Extract only once a drag actually starts (pointer moved past threshold while held), then call `startDrag`. Reuse an already-extracted cache file instead of rewriting it.
- **Network paths**: users do keep libraries on NAS/mapped drives. drag-rs issue #72 panics natively on UNC and mapped-drive paths (uncatchable from JS). So the Rust-side "prepare drag" command copies network files into the local drag cache too, and the plugin only ever sees local paths.
- Drag cache lives in the app's cache dir (not %TEMP%), original filename preserved.

## Implementation (awaiting manual test)
- Uses the `drag` crate (drag-rs core, 2.1.1) directly from our own command instead of `tauri-plugin-drag`. That way preparing the file and starting the drag is one invoke, we can check the button is still held before starting the drag (on Windows, a drag begun after release is an instant drop), and `catch_unwind` guards the crate's `.unwrap()`s.
- `start_asset_drag(assetId, image)` in `src-tauri/src/commands/external.rs`. Plain local files are used in place. ZIP/nested-ZIP entries and network files (UNC, `\?\UNC\`, drives where `GetDriveTypeW` = DRIVE_REMOTE) go to `<app data>/drag-cache/<id>_<fs_mtime>/<original filename>`, written via `.partial` + rename and reused if already there. Entries unused for 7 days are pruned at startup; reuse refreshes the mtime.
- Frontend: `dragOut(asset)` attachment in `assetActions.ts`, on AudioList rows, ImageGrid tiles, AssetList rows. It starts after moving 6 px with the button held, listens on window, and suppresses the webview's own `<img>` dragstart. The drag preview is a themed canvas PNG label (♪ + filename).
- Harness-verified (synthetic input, so the backend correctly answers `released` and no OS drag starts): ZIP and nested-ZIP entries are extracted with valid RIFF/PNG, plain files write nothing, a normal click triggers no drag, the image tile drag reaches the backend, no console errors, e2e 15/15.
- Caveat: a DAW that *references* media in place (e.g. Reaper without "copy media to project") would lose a ZIP-extracted file after the 7-day prune. Audacity copies on import, so it's unaffected.

## Summary of Changes
Native drag-out of single rows/tiles to other programs (see Implementation above). Peter confirmed in the real app: plain files and ZIP entries drag into Audacity/Explorer correctly. The network-drive path (local copy) could not be tested without a NAS; watch for user reports.
