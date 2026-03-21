# CLAUDE.md

## Project Overview

Desktop asset management app: **Tauri 2 (Rust)** + **SvelteKit 2 (Svelte 5)** + **Tailwind CSS 4** + **Vite 6**

## Development Commands

```bash
npm run check:svelte    # Frontend TS errors
npm run check:vite      # CSS/bundling issues
npm run check:cargo     # Backend Rust errors
```

**Do not run other commands.** Ask the user to test the application.

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
