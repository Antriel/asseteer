---
# asseteer-8njz
title: 'Nested ZIP: clean up debug instrumentation and restore production timeout behavior'
status: completed
type: task
priority: low
created_at: 2026-03-19T16:21:56Z
updated_at: 2026-03-20T06:35:50Z
parent: asseteer-kvnt
---

Remove or reduce the temporary ZipGate/ZipCache/AudioProcess debug logging added during nested-ZIP investigation, and re-evaluate the temporary disabled timeout path for nested-ZIP audio now that the active-key coordinator has fixed cache thrash. Keep enough observability for future regressions without flooding logs.


## Implementation

- [x] Reduce `zip_cache.rs` logging: removed all per-operation `[ZipGate]` and `[ZipCache]` println logs; replaced with `eprintln` warnings only when waits exceed 5s or loads exceed 10s
- [x] Reduce `processor.rs` logging: removed per-asset `[AudioProcess] START/LOAD/DONE` logs; warn only when load >10s or probe >5s
- [x] Reduce `work_queue.rs` logging: removed per-batch nested key logs, per-worker start/finish/skip logs; kept failure logging (as eprintln) and category-level summary logs
- [x] Keep timeout bypass for nested ZIP audio (no functional change)


## Summary of Changes

Converted all nested-ZIP debug instrumentation from per-operation `println!` to threshold-based `eprintln!` warnings:
- **zip_cache.rs**: `[ZipGate]` and `[ZipCache]` logs only emit when waits >5s or loads >10s
- **processor.rs**: `[AudioProcess]` logs only emit when audio load >10s or probe >5s
- **work_queue.rs**: Removed per-batch/per-worker lifecycle logs; kept failure warnings and category summary logs
- No functional changes to timeout behavior or processing logic
