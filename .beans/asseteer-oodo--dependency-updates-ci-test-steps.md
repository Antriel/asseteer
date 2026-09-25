---
# asseteer-oodo
title: Dependency updates + CI test steps
status: todo
type: task
priority: normal
created_at: 2026-09-25T08:09:14Z
updated_at: 2026-09-25T08:09:14Z
---

Returning after ~5 months; bring deps current and make CI actually test.

## npm (as of 2026-09-25, `npm outdated`)
In-range, low risk: svelte 5.43→5.57, @sveltejs/kit 2.48→2.70, @tauri-apps/api+cli 2.9→2.11, plugin-dialog 2.4→2.7, plugin-sql 2.3→2.4, plugin-opener, tailwindcss + @tailwindcss/vite 4.1→4.3, svelte-check 4.3→4.7, prettier.
Majors — decide separately, may defer: vite 7→8, @sveltejs/vite-plugin-svelte 6→7, typescript 5.9→7, prettier-plugin-svelte 3→4.

## Cargo
tauri 2.9.5 → latest 2.x. Audit: zip 2.4, reqwest 0.12, rusqlite 0.32, windows-sys 0.59, symphonia 0.5, fast_image_resize 5.
**Constraint**: rusqlite and sqlx both link `libsqlite3-sys` — versions must resolve to one libsqlite3-sys, bump them together.

## CI (`.github/workflows/build.yml`)
Currently only builds. Add: `cargo test`, `npm run check:svelte`, `npm run test:unit` (vitest, added with the test harness).

## Verify
Run the CDP harness (`tests/CLAUDE.md`) after bumps — screenshot library/processing/settings views, console-error gate clean.

- [ ] npm in-range updates
- [ ] decide on majors
- [ ] cargo updates (tauri, audit list)
- [ ] CI test steps
- [ ] verify via harness
