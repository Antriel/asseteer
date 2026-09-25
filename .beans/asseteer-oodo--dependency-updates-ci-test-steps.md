---
# asseteer-oodo
title: Dependency updates + CI test steps
status: completed
type: task
priority: normal
created_at: 2026-09-25T08:09:14Z
updated_at: 2026-09-25T08:59:33Z
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

- [x] npm in-range updates
- [x] decide on majors — deferred: zip → asseteer-n6zt (only if perf gains), rest → asseteer-xtmb
- [x] cargo updates (tauri, audit list) — in-range done; majors listed below
- [x] CI test steps
- [x] verify via harness

## Progress (2026-09-25)

- `npm update` + `cargo update` (lockfiles only). tauri 2.9.5 → 2.11.6; npm @tauri-apps/* moved in lockstep (dialog 2.7.3, opener 2.5.5, sql 2.4.1) — the CLI rejects major.minor mismatches, so npm and crate bumps must go together.
- CI: new `test` job (ubuntu-22.04) — svelte-kit sync + check:svelte, test:unit, `npm run build` (generate_context! needs build/), test:cargo. `build` now `needs: test`.
- Fixed flaky `test_queue_processes_all_images`: `wait_for_category_completion` returned on counters alone, but workers bump `completed` before sending to the batch writer. Now also waits for `!is_running` (set by the completion monitor after flush). Test-only helper.
- Fixed harness flake: vite watcher crashed with EBUSY on WebView2's locked temp files in `tests/harness/out/profile`. Added harness out + fixture library to `server.watch.ignored`.
- Verified: check:svelte 0 errors, test:unit 5/5, cargo test 71/71 (repeated under load), e2e 5 pass + 1 known fixme, screenshots of library/processing/settings both themes, no console errors.

## Remaining majors (held back)

npm: vite 8, @sveltejs/vite-plugin-svelte 7 (go together), typescript 7, prettier-plugin-svelte 4.
Cargo: rusqlite 0.32→0.40 + sqlx 0.8→0.9 (together, one libsqlite3-sys; sqlx 0.9 needs Rust 1.94), zip 2→8, reqwest 0.12→0.13, symphonia 0.5→0.6, fast_image_resize 5→6, windows-sys 0.59→0.61, base64 0.23.

## Summary of Changes

In-range npm + Cargo updates (lockfiles only; tauri 2.9.5 → 2.11.6 with matching @tauri-apps/* npm packages). CI gained a `test` job (svelte-check, vitest, frontend build, cargo test) that gates `build`. Fixed two flakes that would have made CI unreliable: a batch-writer flush race in the Rust test helper `wait_for_category_completion`, and vite crashing on EBUSY while watching the harness's WebView2 profile. Majors deferred to asseteer-n6zt (zip) and asseteer-xtmb (rest).
