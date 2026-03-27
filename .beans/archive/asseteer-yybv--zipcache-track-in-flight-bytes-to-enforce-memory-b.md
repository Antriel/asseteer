---
# asseteer-yybv
title: 'ZipCache: track in-flight bytes to enforce memory budget during parallel scan'
status: completed
type: bug
priority: normal
created_at: 2026-03-23T11:30:57Z
updated_at: 2026-03-23T11:34:39Z
---

Parallel scan loads bypass memory budget because in-flight (Loading) entries aren't counted. Peak 18GB cached against 8GB budget.


## Summary of Changes

**Root cause:** `get_or_load_cached_bytes` didn't account for in-flight (Loading) entries when checking the memory budget. With parallel scan, ~20 threads would all pass the budget check simultaneously because `total_cached_bytes` only counted completed (Ready) entries, not the concurrent decompression operations. This caused peak memory usage of 18 GB against an 8 GB budget.

**Fix in `zip_cache.rs`:**
- Added `in_flight_bytes` field to `ZipCacheInner` — tracks estimated size of all entries currently being decompressed
- Cache miss path now checks `total_cached_bytes + in_flight_bytes + estimated_size > budget_bytes`; if over budget, **waits on condvar** until space opens up (creates natural backpressure)
- On load completion: releases reservation (`in_flight_bytes -= estimated`) and adds actual size
- `evict_for_budget` now considers `in_flight_bytes` in its budget calculation
- `load_for_scan` accepts a `size_hint: u64` parameter for accurate per-entry budget reservation

**Fix in `scan.rs`:**
- Passes nested ZIP's uncompressed size (from `entry.size()`) as `size_hint` to `load_for_scan`
- Replaces the 1 GB default estimate with actual entry metadata — small ZIPs (10 MB) only reserve 10 MB, allowing more concurrent small loads while staying within budget
