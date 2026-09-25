---
# asseteer-xtmb
title: Major dependency upgrades (deferred)
status: todo
type: task
priority: deferred
created_at: 2026-09-25T08:59:13Z
updated_at: 2026-09-25T08:59:13Z
---

Majors held back from asseteer-oodo (2026-09-25). zip is tracked separately.

npm:
- vite 7 → 8 together with @sveltejs/vite-plugin-svelte 6 → 7
- typescript 5.9 → 7 (native compiler; check svelte-check support first)
- prettier-plugin-svelte 3 → 4

Cargo:
- rusqlite 0.32 → 0.40 together with sqlx 0.8 → 0.9 (must resolve to one libsqlite3-sys; sqlx 0.9 needs Rust 1.94)
- reqwest 0.12 → 0.13, symphonia 0.5 → 0.6, fast_image_resize 5 → 6, windows-sys 0.59 → 0.61, base64 0.22 → 0.23
