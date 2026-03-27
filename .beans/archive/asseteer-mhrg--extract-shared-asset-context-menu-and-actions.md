---
# asseteer-mhrg
title: Extract shared asset context menu and actions
status: completed
type: task
priority: high
created_at: 2026-03-20T11:43:24Z
updated_at: 2026-03-21T07:46:26Z
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

## Todo

- [x] Create `src/lib/actions/assetActions.ts`
- [x] Create `src/lib/components/shared/AssetContextMenu.svelte`
- [x] Update AudioList.svelte
- [x] Update ImageGrid.svelte
- [x] Update AssetList.svelte
- [x] Update CLAUDE.md

## Summary of Changes

- Created `src/lib/actions/assetActions.ts` with `showInFolder(asset, assetType)` and `openDirectory(asset)` — the zip-prefix logic now lives in one place
- Created `src/lib/components/shared/AssetContextMenu.svelte` — handles backdrop, positioning, and standard menu items; accepts an `extraItems` snippet for per-component extras
- Refactored all 3 components to use the shared utilities; AudioList passes a `extraItems` snippet for its "Find Similar Sounds" item
- Updated `CLAUDE.md` to document the new shared patterns
