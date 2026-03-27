---
# asseteer-9aq2
title: Size-proportional ZipCache slow LOAD warning threshold
status: completed
type: task
priority: normal
created_at: 2026-03-24T07:33:53Z
updated_at: 2026-03-24T07:38:30Z
---

Replace the fixed 10-second CACHE_LOAD_WARN_MS threshold with a size-proportional formula that accounts for both a fixed seek/open overhead and a per-MB decompression rate.

Example: `warn_threshold_ms = SEEK_MS + size_mb / MIN_THROUGHPUT_MB_S * 1000`

The constants should be conservative to avoid false warnings on slower CPUs and HDDs. Current data point: 640 GB import on fast hardware shows ~40-80 MB/s decompression throughput, so MIN_THROUGHPUT should be well below that (e.g. 15-25 MB/s). SEEK_MS covers ZIP open + central directory read overhead (e.g. 2-5 seconds).

Current code: `zip_cache.rs` line 40 (`CACHE_LOAD_WARN_MS = 10000`) and line 346 (`if load_ms > CACHE_LOAD_WARN_MS`).

## Summary of Changes

Replaced fixed `CACHE_LOAD_WARN_MS = 10000` constant with a size-proportional formula:

- `CACHE_LOAD_SEEK_MS = 4000` — fixed overhead for ZIP open + central directory read
- `CACHE_LOAD_MIN_THROUGHPUT_MB_S = 15.0` — conservative floor (real hardware: 40-80 MB/s)
- `cache_load_warn_threshold_ms(size_bytes)` — computes `SEEK_MS + size_mb / MIN_THROUGHPUT * 1000`

This prevents false warnings for large ZIPs on slower hardware while still catching genuinely unexpected slowness.
