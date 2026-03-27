---
# asseteer-klss
title: 'Nested ZIP import: improve initial scan/import throughput'
status: scrapped
type: task
priority: normal
created_at: 2026-03-19T16:21:56Z
updated_at: 2026-03-20T06:35:25Z
parent: asseteer-kvnt
---

The processing-stage nested-ZIP slowdown is fixed, but the initial importing stage that enumerates nested ZIP contents is still serial and slower than desired. Review scan/import nested-ZIP traversal and improve throughput without reintroducing OOM risk.


## Analysis (2026-03-20)

### Current architecture
- **Scan/import** runs single-threaded: `WalkDir` on one blocking thread, nested ZIP enumeration serial within that thread
- **Audio/image metadata processing** is fully parallel: `num_cpus - 1` workers, concurrency effectively unlimited for image/audio
- **Nested ZIP processing** is coordinated: one active ZIP key at a time, but multiple workers read from shared cached bytes in parallel

### Improvement opportunities
1. **Parallel directory walking**: Replace `WalkDir` with `jwalk` for parallel FS traversal (gains on deep folder trees)
2. **Parallel ZIP enumeration**: Process multiple ZIP files concurrently using a thread pool within the discovery phase
3. **Multi-value INSERT batching**: Currently one INSERT per asset in a transaction; multi-row VALUES would reduce SQLite overhead
4. **Nested ZIP enumeration**: Inherently serial per outer ZIP (must read nested ZIP into memory from parent archive), limited gains possible


## Investigation Results (2026-03-20)

### Benchmarks
- **28.5 GB nested ZIP bundle** (45 outer ZIPs, each with 1 nested ZIP, 5600 assets): ~1 minute from both SATA SSD (500 MB/s) and NVMe SSD (7 GB/s) — same time on both, confirming CPU-bound bottleneck
- **21 GB non-nested ZIP bundle** (2900 assets): ~2 seconds from NVMe SSD
- **1 GB non-ZIP folder** (70k assets): ~10 seconds

### Root cause: Deflate decompression of outer ZIPs
The outer ZIPs use Deflate compression, so each nested ZIP (~633 MB avg) must be fully decompressed into memory before its entries can be enumerated. This takes ~1.3s per nested ZIP × 45 = ~60s total, done serially. Non-nested ZIPs use Store method (no compression), so enumeration is near-instant — just index parsing.

### Why no change is warranted
- Parallel ZIP enumeration across outer ZIPs would give linear speedup with CPU cores, but the import is a one-time operation and 1 minute is acceptable
- The actual asset processing (audio metadata, CLAP embeddings) takes much longer than import
- Compression method is determined by the ZIP creator, not something we control
- Multi-row INSERT and parallel directory walking showed no meaningful gains in benchmarks

## Reasons for Scrapping
Import throughput is already near-optimal for the hardware. The nested ZIP bottleneck is CPU-bound Deflate decompression which is inherent to the ZIP format and not worth parallelizing for a one-time ~1 minute operation.
