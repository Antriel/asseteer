# Asseteer design system

## Direction

A sound/image browser for a game developer auditioning hundreds of assets with the
keyboard. It should feel like a DAW's browser panel: **dense, quiet, content-first**. Chrome
recedes; filenames and the playing sound are the only things that should draw the eye.
Dark theme is the primary one; light must still work (the app follows
`prefers-color-scheme`, check both).

Prefer density over decoration: no cards around list items, no decorative icons (an icon
that repeats on every row carries no information), no gradients.

## Tokens (`src/app.css`)

GitHub-derived neutrals, one accent (blue). Purple is reserved for semantic/CLAP search
(similarity %, Semantic toggle, similar-sounds actions) — it means "AI", never decoration.

| Role | Class | Notes |
|---|---|---|
| Canvas | `bg-primary` | lists, main content |
| Chrome | `bg-secondary` | nav sidebar, folder panel, toolbar, transport strip — all the same, separated by `border-default` |
| Hover / raised | `bg-tertiary` | button hover, badges |
| Text | `text-primary` › `text-secondary` › `text-tertiary` | name › key value (duration) › metadata |
| Hairline | `border-subtle` | row dividers (rgba, ~6%) |
| Structure | `border-default` | panel edges, control outlines |
| Scrubber track | `bg-track` | unfilled part of progress/meters |
| Selection | `bg-accent-light` + 2px `bg-accent` left edge | rows, active segment |

Don't use classes that aren't defined — they silently render nothing (`bg-default` was the
invisible-progress-bar bug). Only `accent` / `accent-hover` are registered in Tailwind's
`@theme`, so `ring-accent`, `bg-accent/10`, `hover:bg-accent/90` work; the other tokens
exist only as the hand-written classes in `app.css` (no opacity modifiers, no `ring-*`).

## Depth

Borders + surface shifts only. No shadows except tiny ones on popovers/thumbs. Surfaces are
flat — no gradients.

## Spacing & sizes

- Base unit 4px (Tailwind scale). Panels `px-4`; toolbar `py-3`.
- **Control height 36px (`h-9`)** for everything in a toolbar row: buttons, inputs, segmented
  controls. Small controls inside a strip: `h-7` / `h-6`.
- **List rows 32px (`h-8`)**, single line, `border-b border-subtle`. `VirtualList itemHeight`
  must match exactly.
- Radius: `rounded-md` for controls, `rounded` for inner segments/badges, `rounded-full` only
  for the play button and scrubber thumb.

## Typography

System UI stack. Body `text-sm`; metadata `text-xs`; tiny labels `text-[11px]`/`text-[10px]`.
Numbers that line up in columns or tick while playing use `tabular-nums`.
Durations in lists: `formatDurationCompact` ("0.35 s", "12.5 s", "1:05"). The player keeps
`formatDuration` (ms precision).

## Responsive

Size by the **panel**, not the window: the folder panel eats width independently. Use
Tailwind container queries — `@container` on the region, `@xl:` / `@2xl:` / `@3xl:` on
children. Order of shedding:

- List rows: size (`@xl`), then sample rate + channels (`@3xl`). Filename and duration never go.
- Transport strip: metadata (`@3xl`), volume (`@2xl`), Test loop (`@xl`), idle hint (`@2xl`).
- Toolbar (the search field is the priority element): Folders and inactive Duration are
  icon-only below `@5xl`, Semantic below `@4xl`, Audio/Images below `@3xl`; result count
  hides below `@2xl`; the row `flex-wrap`s rather than squeezing the search field below
  `min-w-72`.

## Patterns

**Library header**: a single toolbar row — folder-panel toggle · Audio/Images segmented
switch · search field · Semantic · Duration · view toggles · result count. No tab bar, no
filter banners. Library totals (hundreds of thousands) go in the switch's tooltip, never in
labels.

**Search field = everything that decides what matches**: one bordered box
(`focus-within:ring-accent/60`, purple in semantic mode) holding, in order: search
icon/spinner · similarity chip (purple) · folder-scope chip (neutral, deepest segment,
full location in the tooltip) · the bare input · clear × · Name/Path restriction toggles.
Chips never shrink below their label (`flex-shrink-0 max-w-36`); Backspace in an empty input
removes the nearest chip. The Name/Path toggles are *optional restrictions*
(`aria-pressed`, mutually exclusive, click again to clear): neither pressed = both, which is
the quiet default — never make the user turn a toggle *off* to narrow a search. The
placeholder states the current scope ("Search audio…" / "Search audio names…").

**Segmented control** (search scope, end-of-track mode): a `radiogroup`, outer
`p-0.5 bg-primary border border-default rounded-md`; segments `rounded`, active
`bg-accent-light text-accent`, inactive `text-tertiary hover:text-primary`; `role="radio"` +
`aria-checked`. Use it instead of native `<select>` and instead of buttons that cycle
through hidden states.

**List row** (`AudioList.svelte`): leading 16px glyph column (play on hover, play/pause when
selected) · filename (`max-w-[60%]`, truncates) · path relative to the source
(`getAssetRelativeDirectory`, `text-xs text-tertiary`, truncates from the left via
`[direction:rtl]` + `<bdi>`) · badges · fixed-width right-aligned metadata columns.
Selected row carries the **playhead wash** — an `opacity-10` accent fill at the playback
fraction — this is the audio view's signature; keep it.

**Transport strip**: docked, fixed height (68px) whether idle or playing, so the list never
jumps. Play button (36px accent circle) · name line + scrubber line · controls separated by
`border-l` · volume. Idle state shows keyboard hints with `<kbd>`.

**Scrubber**: 16px hit area, 4px `bg-track` track growing to 6px on hover/drag, accent fill,
thumb dot visible on hover/drag; pointer capture for dragging.

**Toolbar toggles**: active state = `bg-accent-muted text-accent` (Folders) — not solid
accent fills. (`ViewModeToggle` still uses solid `bg-accent`; migrate it to the segmented
pattern when touched.)
