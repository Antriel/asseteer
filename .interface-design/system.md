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

Don't use classes that aren't defined in `app.css` (e.g. `bg-default`) — they silently render
nothing (that was the invisible-progress-bar bug).

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
- Toolbar: text labels collapse to icons below `@3xl` (Folders, Semantic, inactive Duration),
  result count hides below `@2xl`, and the row `flex-wrap`s rather than squeezing the search
  box below `min-w-48`.

## Patterns

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
