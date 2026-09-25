---
# asseteer-2ur6
title: Ctrl+C copies selected files to the OS clipboard
status: completed
type: feature
priority: normal
created_at: 2026-09-25T11:51:36Z
updated_at: 2026-09-25T11:55:55Z
---

Ctrl+C in the audio list / image grid / image list puts the selected files on the Windows clipboard (CF_HDROP + Preferred DropEffect=copy), so Ctrl+V pastes real files in Explorer or a DAW. ZIP entries and network files are materialized to drag-cache first (shared with drag-out via `local_files`). A "Copy  Ctrl+C" item is in the context menu; Copy Path now uses a link icon.

Ctrl+V into Asseteer is out of scope (nothing to paste into). Audacity's Ctrl+V only pastes its own audio clipboard, so this helps Explorer / Reaper and similar, not Audacity.

Harness-verified by reading the real clipboard back through PowerShell: 2 files (ZIP entries extracted) with effect=1; plain files with backslash paths; image Ctrl+C with a selection; Ctrl+C in the search box still copies text. Both themes; e2e/unit/cargo green.

- [x] backend command + clipboard format
- [x] Ctrl+C in lists, context menu item
- [x] Manual: Ctrl+C in Asseteer → Ctrl+V in Explorer (plain + zipped)

## Summary of Changes
Ctrl+C / context-menu Copy put the selected files on the OS clipboard as CF_HDROP with a copy drop effect; ZIP entries and network files go through the drag cache first. Peter confirmed pasting into Explorer works.
