# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A desktop application built with Tauri, Svelte, TypeScript, and Tailwind CSS.
- **Frontend**: SvelteKit 2 (Svelte 5) + TypeScript + Tailwind CSS 4
- **Backend**: Tauri 2 (Rust)
- **Build Tool**: Vite 6

## Development Commands

```bash
# Check for frontend TS errors
npm run check:svelte
# Check for frontend Vite errors (CSS and bundling issues)
npm run check:vite
# Check for backend Rust errors
npm run check:cargo
```

Do not run other commands! If you want to test the application, ask the user to do it.

## Architecture

### Frontend (SvelteKit)
- **Entry Points**:
  - `src/app.html` - HTML template
  - `src/routes/+layout.svelte` - Root layout
  - `src/routes/+page.svelte` - Main application page

- **State Management**: Svelte 5 runes-based state in `src/lib/state/`
  - State modules in `.svelte.ts` files
  - Use `$state`, `$derived`, `$effect` runes
  - Export state directly, use getter functions for derived values

- **Components**: Organized by feature in `src/lib/components/`
  - `shared/` - Reusable components
  - `layout/` - Layout components
  - Feature-specific component folders

### Backend (Tauri/Rust)
- **Entry Point**: `src-tauri/src/main.rs` → `src-tauri/src/lib.rs`
- **Commands**: Organized in `src-tauri/src/commands/`
  - `scan.rs` - Asset discovery and file scanning
  - `search.rs` - Asset search and retrieval
  - `process.rs` - **Asset processing pipeline (images and audio)**
- **Database Layer**: Organized in `src-tauri/src/database/`
  - SQLite with sqlx for async operations
  - FTS5 for full-text search
  - Connection pooling for performance
- **Models**: Data structures in `src-tauri/src/models.rs`

### Asset Processing Pipeline

The application processes discovered assets in two phases:

**Phase 1: Discovery** (`scan.rs`)
- Recursively scans directory for supported file types
- Inserts files into database with `processing_status = 'pending'`
- Supported formats:
  - Images: PNG, JPG, JPEG, WebP, GIF, BMP
  - Audio: MP3, WAV, OGG, FLAC, M4A, AAC

**Phase 2: Processing** (`process.rs`)
- **Image Processing**:
  - Extracts dimensions (width × height)
  - Generates 128px JPEG thumbnails using `fast_image_resize` (Lanczos3 filter)
  - Stores thumbnail as BLOB in database (<20KB per image)
  - Uses Rayon for parallel batch processing (50 files/batch)

- **Audio Processing**:
  - Extracts metadata using Symphonia library
  - Duration (milliseconds), sample rate (Hz), channel count
  - No thumbnail generation for audio files

**Tauri Commands** (exposed to frontend):
```rust
// Process all pending image assets (thumbnails + metadata)
process_pending_images() -> Result<usize, String>

// Process all pending audio assets (metadata only)
process_pending_audio() -> Result<usize, String>

// Retrieve thumbnail for specific asset
get_thumbnail(asset_id: i64) -> Result<Vec<u8>, String>
```

**Progress Tracking**:
- Emits `process-progress` events during processing
- Frontend listens via `@tauri-apps/api/event`
- Real-time UI updates for user feedback

**Database Updates**:
- Uses sqlx transactions for atomic batch updates
- On success: Sets `processing_status = 'complete'`, writes metadata/thumbnail
- On error: Sets `processing_status = 'error'`, stores error message

## Tech Stack

### Desktop Framework
- **Tauri v2**: Rust-based desktop framework with native webviews
  - Small bundle size (3-5MB)
  - Low memory footprint
  - Built-in plugins for fs, dialog, and other operations

### Frontend
- **Svelte 5**: Modern reactive framework using **runes** (`$state`, `$derived`, `$effect`)
  - **IMPORTANT**: Use Svelte 5 runes syntax, NOT legacy `$:` reactive statements
  - **SvelteKit** with adapter-static (SPA mode) - idiomatic Tauri + Svelte setup
  - TypeScript with strict mode
  - Vite as build tool

### Styling
- **Tailwind CSS 4**: Utility-first CSS framework
  - Light/dark mode support (system preference + manual toggle)
  - CSS variables for theming
  - Inline-first approach: Use inline Tailwind classes by default, custom utilities only for truly reusable patterns used in 3+ components

## Key Technical Decisions

### 1. Svelte 5 Runes (IMPORTANT)
**Use modern Svelte 5 syntax:**

```svelte
<script lang="ts">
  // ✅ CORRECT - Use runes
  let count = $state(0);
  let doubled = $derived(count * 2);

  $effect(() => {
    console.log(`Count is now ${count}`);
  });

  // ❌ WRONG - Don't use legacy syntax
  // let count = 0;
  // $: doubled = count * 2;
  // $: console.log(count);
</script>
```

**State modules pattern:**
```typescript
// src/lib/state/example.svelte.ts
import type { MyType } from '$lib/types';

// ✅ CORRECT - Export $state directly
export const myState = $state<MyType>({ value: 0 });

// ✅ CORRECT - Export getter functions for derived values
export function getDerivedValue(): number {
  return myState.value * 2;
}

// ❌ WRONG - Cannot export $derived directly from modules
// export const derivedValue = $derived(myState.value * 2);
```

**Component usage:**
```svelte
<script lang="ts">
  import { myState, getDerivedValue } from '$lib/state/example.svelte';

  // Access state directly - no subscriptions needed!
  const doubled = $derived(getDerivedValue());
</script>

<p>Value: {myState.value}</p>
<p>Doubled: {doubled}</p>
```

### 2. Tailwind CSS Philosophy: Inline-First Approach

**Core principle:** Use inline Tailwind classes by default. Only create custom `@utility` classes for patterns used across **3+ components**.

**State Management**:
- Export `$state` objects directly from `.svelte.ts` modules
- Export **functions** for derived values (not `$derived` directly)
- Key pattern: Direct export of state, function exports for derivations

**Important Notes:**
- Reactivity with Maps: Use `SvelteMap` from `svelte/reactivity`
- Accessing reactive state: Just accessing a property does NOT trigger reactivity - use proper Svelte primitives

**When to create a custom utility:**
```
✅ CREATE utility if:
  - Pattern used in 3+ components
  - Truly reusable (buttons, forms, modals)

❌ DON'T create utility if:
  - Component-specific styling
  - Used in 1-2 places only
  - One-off patterns
```

**Theming with CSS variables:**
```css
/* app.css - Theme variables */
@theme {
  --color-bg-primary: #ffffff;
  --color-bg-secondary: #f9fafb;
  --color-text-primary: #111827;
  --color-text-secondary: #6b7280;
  --color-border: #e5e7eb;
  --color-accent: #3b82f6;
}

/* Semantic color classes */
.text-primary { color: var(--color-text-primary); }
.bg-primary { background-color: var(--color-bg-primary); }
.border-default { border-color: var(--color-border); }
```

**Component examples:**
```svelte
<!-- ✅ CORRECT - Use semantic color classes -->
<div class="flex items-center gap-2 px-4 py-2 bg-secondary border-b border-default">
  <span class="text-sm font-medium text-secondary">Label:</span>
  <span class="text-sm font-semibold text-primary">{value}</span>
</div>

<!-- ❌ WRONG - Never use component <style> blocks -->
<button class="custom-btn">Click</button>
<style>
  .custom-btn { ... }  /* Don't do this! */
</style>
```

**Benefits:**
- Token efficient - All styling visible inline
- Better for AI/LLM understanding
- Self-documenting code
- Less maintenance

⚠️ **IMPORTANT:** Never use component-specific `<style>` blocks in Svelte components!

### 3. Tauri Backend Integration

**Use built-in plugins wherever possible:**
```typescript
import { save, open } from '@tauri-apps/plugin-dialog';
import { writeFile, readTextFile } from '@tauri-apps/plugin-fs';

// File operations
async function saveData(data: any, path: string) {
  const json = JSON.stringify(data, null, 2);
  await writeFile(path, json);
}
```

### 4. Error Handling & User Feedback

**⚠️ IMPORTANT**: Tauri restricts native browser dialogs (`alert()`, `confirm()`) for security. Always use custom UI components.

#### Toast Notifications
**When to use**: Non-blocking feedback for operations (success, error, warning, info)

```typescript
import { showToast } from '$lib/state/ui.svelte';

// Success notification
showToast('Operation completed successfully', 'success');

// Error notification
showToast('Failed to save: ' + error, 'error');
```

#### Confirmation Dialogs
**When to use**: Destructive actions requiring user confirmation

```typescript
import { showConfirm } from '$lib/state/ui.svelte';

// Confirmation dialog (returns Promise<boolean>)
async function handleDelete() {
  const confirmed = await showConfirm(
    'Delete this item? This action cannot be undone.',
    'Confirm Deletion',
    'Delete'
  );

  if (confirmed) {
    // User confirmed
    deleteItem();
  }
}
```

#### Best Practices
```typescript
// ✅ CORRECT - Use toast for feedback
try {
  await saveData();
  showToast('Saved successfully', 'success');
} catch (error) {
  showToast('Save failed: ' + error, 'error');
}

// ✅ CORRECT - Use confirm for destructive actions
const confirmed = await showConfirm(
  'Discard changes?',
  'Unsaved Changes',
  'Discard'
);

// ❌ WRONG - Never use native dialogs
alert('Blocked by Tauri!');  // Security error
```

## File Structure Conventions

```
src/
├── lib/
│   ├── components/       # Svelte components
│   │   ├── shared/       # Reusable components
│   │   ├── layout/       # Layout components
│   │   └── [feature]/    # Feature-specific components
│   ├── state/            # Svelte 5 rune-based state modules (.svelte.ts)
│   │   ├── ui.svelte.ts        # UI state (toasts, modals, etc.)
│   │   └── [feature].svelte.ts # Feature-specific state
│   ├── types/            # TypeScript interfaces
│   ├── utils/            # Helper functions
├── routes/               # SvelteKit routes (SPA mode)
│   ├── +layout.svelte   # Root layout
│   ├── +page.svelte     # Main page
│   └── [route]/         # Other routes
├── app.html             # HTML template
└── app.css              # Global styles with theme variables
src-tauri/
├── src/
│   ├── commands/        # Tauri command handlers
│   ├── database/        # Database layer (if applicable)
│   ├── services/        # Business logic
│   └── utils/           # Helper functions
├── migrations/          # Database migrations
└── tauri.conf.json      # Tauri configuration
```

## Code Style Guidelines

### Svelte 5 Component Template
```svelte
<script lang="ts">
  import type { MyType } from '$lib/types';

  interface Props {
    item: MyType;
    disabled?: boolean;
  }

  let { item, disabled = false }: Props = $props();

  // State
  let isActive = $state(false);

  // Derived values
  let isDisabled = $derived(disabled || !item.enabled);

  // Effects
  $effect(() => {
    console.log('Item changed:', item.id);
  });

  // Event handlers
  function handleClick() {
    if (isDisabled) return;
    isActive = !isActive;
  }
</script>

<div class="bg-primary p-4 border border-default">
  <!-- Component content -->
  <!-- Use Tailwind utilities or custom utilities from app.css -->
  <!-- NO <style> blocks allowed in components! -->
</div>
```

### TypeScript
- Use strict mode
- Prefer interfaces over types for objects
- Use type for unions and primitives
- Explicit return types for public functions

### Naming Conventions
- Components: PascalCase (e.g., `MyComponent.svelte`)
- Files: camelCase (e.g., `helperFunction.ts`)
- State modules: camelCase with `.svelte.ts` extension (e.g., `myFeature.svelte.ts`)
- State objects: camelCase (e.g., `myState`, `ui`, `data`)
- CSS variables: kebab-case (e.g., `--color-bg-primary`)
- Functions: camelCase, descriptive verbs (e.g., `handleSubmit`, `formatData`)

### State Management Pattern
- State exported directly: `export const myState = $state(initialValue)`
- Derived values as functions: `export function getDerivedValue()`
- Component-local state: `let localState = $state(value)`
- Component-derived: `let derived = $derived(calculation)`

## Testing Strategy

### Test Philosophy
- Test behavior and contracts, not implementation details
- Test critical integration points (Tauri commands, database operations)
- Focus on catching real breaks

### Backend Tests (Rust)
- Location: `src-tauri/tests/`
- Unit tests for pure functions
- Integration tests for multi-component behavior

### Frontend Tests (TypeScript/Vitest)
- Location: `src/tests/`
- Unit tests for utility functions and state logic
- Framework: Vitest with appropriate environment

### Testing Commands
```bash
# Backend tests
cd src-tauri && cargo test

# Frontend tests
npm run test              # Watch mode
npm run test:unit         # Run once
```

## Configuration
- **Vite**: Configured for Tauri with appropriate ports
- **SvelteKit**: Static adapter with SPA fallback
- **Tailwind**: Version 4 with plugin integration
- **Tauri**: Properly configured for the application needs

## Development Notes
- Svelte 5 runes require specific file extensions and patterns
- Use proper reactivity primitives for Maps and other structures

## Type Definitions
- Types defined within respective state or feature modules
- Import types from their source locations
- Central types file only if truly shared across many modules
