# CLAUDE.md

## Project Overview

Desktop asset management app: **Tauri 2 (Rust)** + **SvelteKit 2 (Svelte 5)** + **Tailwind CSS 4** + **Vite 6**

## Development Commands

```bash
npm run check:svelte    # Frontend TS errors
npm run check:vite      # CSS/bundling issues
npm run check:cargo     # Backend Rust errors
npm run test:unit       # Vitest, pure modules (src/**/*.test.ts)
npm run test:cargo      # Rust tests
```

### Then actually verify it

The checks are types; they say nothing about whether the change works. **A change is not
done until you have shown it working.**

```bash
npm run test:e2e        # Playwright drives the REAL app (isolated data) over CDP
npm run harness         # app up with the fixture library, for an interactive session
```

For a change no spec covers — especially any UI change — write a scratch script against
`withAsseteer` from `tests/harness/drive.mjs`, run it, and **look at the screenshots** (Read
the PNGs, starting with `tests/harness/out/contact.png`). Check both themes for visual work.
`tests/CLAUDE.md` has the loop, the `app` helper surface, and what the harness cannot reach.

**Still ask the user for**: `npm run tauri dev` / `npm run build` (interactive/long-running),
anything involving OS drag-and-drop, native dialogs, actual audio output, and CLAP.

## Architecture Overview

- **Frontend**: SvelteKit SPA with direct SQLite reads via Tauri SQL plugin
- **Backend**: Rust for writes, file ops, and heavy processing
- **Database**: Dual-access (frontend reads, backend writes)

See `src-tauri/CLAUDE.md` for backend details, `src/lib/database/CLAUDE.md` for query patterns.

## Svelte 5 Runes (CRITICAL)

Use runes, NOT legacy `$:` syntax:

```svelte
<script lang="ts">
  let count = $state(0);
  let doubled = $derived(count * 2);
  $effect(() => { console.log(count); });
</script>
```

**State modules** (`.svelte.ts` files) use singleton class pattern:
```typescript
class MyState {
  value = $state(0);
  doubled = $derived(this.value * 2); // OK inside class

  setValue(v: number) { this.value = v; }
}

export const myState = new MyState();

// Export FUNCTIONS for derived values needed outside the class
export function getComputedThing(): number {
  return myState.value * 2;
}
```

**Props**: Use `$props()` with interface:
```svelte
<script lang="ts">
  interface Props { item: MyType; disabled?: boolean; }
  let { item, disabled = false }: Props = $props();
</script>
```

**Callbacks over events**: Use callback props (`onSelect`, `onClose`) not `createEventDispatcher`.

## Tailwind CSS: Inline-First

Use inline Tailwind classes. Only create `@utility` for patterns in **3+ components**.

**Never use `<style>` blocks in components.**

Use semantic color classes from `app.css`: `bg-primary`, `text-secondary`, `border-default`, etc.

## Error Handling

Tauri blocks native `alert()`/`confirm()`. Use:

```typescript
import { showToast, showConfirm } from '$lib/state/ui.svelte';

showToast('Saved successfully', 'success');
showToast('Failed: ' + error, 'error');

const confirmed = await showConfirm('Delete?', 'Confirm', 'Delete');
```

## Database Access

- **Frontend**: ALL read operations via `src/lib/database/queries.ts`
- **Backend**: Write operations only (INSERT/UPDATE/DELETE)

See `src/lib/database/CLAUDE.md` for query patterns.

## Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Components | PascalCase | `MyComponent.svelte` |
| Files | camelCase | `helperFunction.ts` |
| State modules | camelCase + `.svelte.ts` | `myFeature.svelte.ts` |
| CSS variables | kebab-case | `--color-bg-primary` |
| Functions | camelCase verbs | `handleSubmit`, `formatData` |

## Key Patterns

- **Reactivity with Maps/Sets**: Use `SvelteMap`/`SvelteSet` from `svelte/reactivity`
- **State singletons**: Class with `$state` properties, exported as singleton instance
- **Tauri plugins**: Use built-in plugins (`@tauri-apps/plugin-dialog`, etc.) over custom commands
- **Tauri events**: Use `listen()` from `@tauri-apps/api/event` for backend→frontend communication. Store `UnlistenFn` and clean up on destroy.
- **CLAP functions**: Semantic search uses `invoke()` commands, not direct SQL — see bottom of `queries.ts`
- **Asset actions**: `showInFolder(asset, assetType)` and `openDirectory(asset)` live in `$lib/actions/assetActions.ts` — use these instead of duplicating the logic
- **Drag-out / copy path / multi-select**: lists own a `ListSelection` (`$lib/state/listSelection.svelte.ts`, Ctrl/Shift+click). `{@attach dragOut(() => selection.targets(assets, asset))}` on a row or tile drags the real files out to other programs (`start_asset_drag` in `src-tauri/src/commands/external.rs` extracts ZIP entries and copies network files to `drag-cache/` only once a drag starts). `copyAssetFiles(assets)` (Ctrl+C / Copy: real files on the OS clipboard) and `copyAssetPaths(assets)` back the shared context menu (pass `targets`).
- **Asset context menu**: `AssetContextMenu.svelte` (in `shared/`) renders the backdrop + menu panel. Pass `onShowInFolder`, `onOpenDirectory`, and optionally an `extraItems` snippet for additional menu items at the top (e.g., AudioList's "Find Similar Sounds")
- **Formatting utilities**: Use `$lib/utils/format.ts` for `formatDuration(ms)`, `formatFileSize(bytes)`, `formatSimilarity(score)` — do not create local copies

## State Modules

All in `src/lib/state/`, initialized as singletons:

| Module | Singleton | Init | Purpose |
|--------|-----------|------|---------|
| `assets.svelte.ts` | `assetsState` | On demand | Search, filtering, asset list |
| `view.svelte.ts` | `viewState` | Immediate | Active tab, layout, lightbox, sidebar |
| `ui.svelte.ts` | `uiState` | Immediate | Toasts, confirm dialog, scan progress |
| `tasks.svelte.ts` | `processingState` | Root layout | Per-category processing progress + control |
| `clap.svelte.ts` | `clapState` | Root layout | CLAP server management + semantic search |
| `explore.svelte.ts` | `exploreState` | On demand | Folder tree navigation + cache |
| `thumbnails.svelte.ts` | (functions) | On import | Thumbnail cache, request batching |
| `settings.svelte.ts` | `settings` | Immediate | Persisted settings (localStorage) |

"Root layout" = `initializeListeners()`/`initialize()` called in `src/routes/+layout.svelte`.

## UI Structure

**Routes** (`src/routes/`):
- `/library` - Asset browser (images/audio tabs, search, folder panel)
- `/processing` - Processing dashboard (per-category cards)
- `/folders` - Source folder management (add/edit/rescan)
- `/settings` - App settings + CLAP setup

**Layout**: Root layout (`+layout.svelte`) has sidebar + folder panel + status bar + toasts + confirm dialog. Processing and CLAP state initialized once here.

**Icons**: Use `$lib/components/icons` (AudioIcon, PlayIcon, PauseIcon, SearchIcon, etc.) instead of inline SVGs.

**Virtual Scrolling**: Use `VirtualList` for simple lists. `ImageGrid`/`AssetList` have specialized implementations.

**Colors**: `bg-primary/secondary/tertiary/elevated`, `text-success/warning/error`, `bg-accent-muted`


## Bash Tips

**CRITICAL: Backticks in beans commands** — When updating bean body content that contains backticks (code snippets, template literals, etc.), you MUST use a heredoc with a QUOTED delimiter to prevent bash command substitution:
```bash
# WRONG - backticks will be interpreted by bash
beans update <id> --body-append "text with \`code\`"
echo "text with \`code\`" | beans update <id> --body-append -

# CORRECT - heredoc with quoted delimiter (<<'EOF' not <<EOF)
beans update <id> --body-append "$(cat <<'EOF'
text with `code` and `backticks`
EOF
)"
```
