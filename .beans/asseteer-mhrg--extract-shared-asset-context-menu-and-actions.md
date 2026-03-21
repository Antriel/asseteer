---
# asseteer-mhrg
title: Extract shared asset context menu and actions
status: todo
type: task
priority: high
created_at: 2026-03-20T11:43:24Z
updated_at: 2026-03-20T11:48:49Z
parent: asseteer-38rb
---

showInFolder(), openDirectory(), and the entire context menu (markup + backdrop + positioning) are copy-pasted across 3 components:

- `src/lib/components/AudioList.svelte` (lines 81-116, 140-159, 393-432)
- `src/lib/components/ImageGrid.svelte` (lines 76-119, 160-189)
- `src/lib/components/AssetList.svelte` (lines 53-95, 126-155)

The FolderLocation building logic in showInFolder is particularly risky — it constructs zip prefixes identically in all 3 places. A bug fix would need to be applied 3 times.

**Suggested approach:**
- Extract a shared `ContextMenu.svelte` component with menu items as slots/snippets
- Extract `showInFolder(asset)` and `openDirectory(asset)` into a shared utility (e.g., `$lib/actions/assetActions.ts`)
- AudioList's context menu has an extra "Find Similar Sounds" item — handle via optional menu items


## CLAUDE.md Updates
When implementing this, update root `CLAUDE.md` to document the new shared ContextMenu component and asset action utilities under the Key Patterns or UI Structure section.
