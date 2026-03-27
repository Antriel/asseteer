---
# asseteer-g1fi
title: Sort CLAP processing queue by ZIP path
status: completed
type: task
priority: low
created_at: 2026-03-16T09:38:28Z
updated_at: 2026-03-18T15:05:36Z
parent: asseteer-526f
blocked_by:
    - asseteer-8yo6
---

Sort CLAP processing queue by ZIP path so files from the same ZIP are consecutive.

**Lower priority than expected**: Benchmark showed ZIP reopen overhead is only 2.8ms/file. Sorting still helps with OS page cache and is trivial to implement, but the impact is marginal. Still worth doing as a free win when touching the query code.


## Summary of Changes
Changed CLAP query ORDER BY from `a.id` to `a.path, a.zip_entry` in `process.rs`. This is now critical (not just a minor optimization) because:
1. Batch processing (8 files at a time) benefits from same-ZIP batches
2. Inner ZIP caching only works when consecutive files come from the same nested ZIP
