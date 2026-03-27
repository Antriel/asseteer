---
# asseteer-ix94
title: Review ZipCache in_flight_mb implementation and WARN slow LOAD warnings
status: completed
type: task
priority: normal
created_at: 2026-03-23T15:43:25Z
updated_at: 2026-03-24T07:33:57Z
---

During large imports, many 'ZipCache] WARN slow LOAD' messages appear, e.g.: size_mb=1004.8 load_ms=18596 entries=8 cached_mb=1005 in_flight_mb=6542. We recently added in_flight_mb tracking and should verify: (1) the in_flight_mb accounting is correct, (2) the warnings are expected/acceptable at this scale, (3) whether the slow loads indicate a real problem or are just noisy.


## Review Findings (2026-03-24)

### in_flight_mb accounting
- Correct during scan (uses real size_hint from ZIP metadata)
- Processing always reserves DEFAULT_ESTIMATED_SIZE (1 GB) via `load_asset_bytes_cached` — inaccurate for multi-GB ZIPs
- Condvar/mutex coordination and saturating_sub on both success/error paths are correct

### WARN slow LOAD messages
- 54 of 213 loads (25%) exceeded 10s threshold — all correlate with file size
- Decompression throughput steady at ~40-80 MB/s — no degradation
- Expected behavior at 640 GB import scale

### Corrupt nested ZIPs
- 5,779.5 MB file loaded then failed EOCD check — dead weight in cache until evicted
- Two smaller corrupt ZIPs handled gracefully

### Potential improvements identified
1. Proactively evict cache entries when nested ZIP fails to open (medium priority)
2. Size-proportional warn threshold instead of fixed 10s (low priority)
3. Cache actual size mapping for processing reload estimates (low priority)
