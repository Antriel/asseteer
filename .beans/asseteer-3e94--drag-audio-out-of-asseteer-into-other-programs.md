---
# asseteer-3e94
title: Drag audio out of Asseteer into other programs
status: todo
type: feature
priority: normal
created_at: 2026-09-25T08:09:37Z
updated_at: 2026-09-25T08:09:37Z
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

- [ ] spike: plugin + plain file
- [ ] zip entry extraction for drag
- [ ] wire into AudioList rows
- [ ] image grid (optional)
- [ ] temp cleanup
- [ ] manual test by Peter (Audacity + Explorer)
