---
# asseteer-8njz
title: 'Nested ZIP: clean up debug instrumentation and restore production timeout behavior'
status: todo
type: task
priority: low
created_at: 2026-03-19T16:21:56Z
updated_at: 2026-03-19T16:22:10Z
parent: asseteer-kvnt
---

Remove or reduce the temporary ZipGate/ZipCache/AudioProcess debug logging added during nested-ZIP investigation, and re-evaluate the temporary disabled timeout path for nested-ZIP audio now that the active-key coordinator has fixed cache thrash. Keep enough observability for future regressions without flooding logs.
