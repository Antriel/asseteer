---
# asseteer-g1fi
title: Sort CLAP processing queue by ZIP path
status: todo
type: task
priority: low
created_at: 2026-03-16T09:38:28Z
updated_at: 2026-03-16T09:50:03Z
parent: asseteer-526f
blocked_by:
    - asseteer-8yo6
---

Sort CLAP processing queue by ZIP path so files from the same ZIP are consecutive.

**Lower priority than expected**: Benchmark showed ZIP reopen overhead is only 2.8ms/file. Sorting still helps with OS page cache and is trivial to implement, but the impact is marginal. Still worth doing as a free win when touching the query code.
