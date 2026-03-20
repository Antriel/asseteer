---
# asseteer-g82f
title: ZIP handle cache for consecutive extractions
status: scrapped
type: feature
priority: deferred
created_at: 2026-03-16T09:39:17Z
updated_at: 2026-03-20T09:57:32Z
parent: asseteer-526f
blocked_by:
    - asseteer-8yo6
---

ZIP handle caching for consecutive extractions.

**DEPRIORITIZED**: Benchmark showed ZIP overhead is only 4% of total processing time (2.8ms out of 78ms per file). Even on a 41MB ZIP with reopen-per-file, the OS page cache makes this negligible. Larger ZIPs (1GB+) may show more overhead, but the win ceiling is small.

Revisit only if benchmarks on larger ZIPs show significantly different numbers.

## Reasons for Scrapping
Parent epic scrapped — not worth extra complexity.
