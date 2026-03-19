---
# asseteer-5m6z
title: Increase CLAP concurrency for I/O pipelining
status: todo
type: task
priority: high
created_at: 2026-03-16T09:38:40Z
updated_at: 2026-03-18T11:17:20Z
parent: asseteer-526f
blocked_by:
    - asseteer-8yo6
---

Increase CLAP concurrency and spawn multiple server processes for parallel inference.

## Benchmark Results
- 1 server, 1 concurrent: 14.9 files/sec (5.3h for 283K)
- 2 servers, 4 concurrent: 33.3 files/sec (2.4h for 283K) -- **2.2x speedup**
- 4 servers barely helps beyond 2

## Implementation Plan
- [ ] Spawn 2 CLAP server processes on different ports (e.g., 5555, 5556)
- [ ] Update Rust client to round-robin requests across server instances
- [ ] Change CLAP max_concurrent from 1 to 4 in work_queue.rs
- [ ] Server lifecycle: start/stop both processes together

## Also fixed during benchmarking
- [x] async def endpoints blocking event loop (changed to sync def)
- [x] clap_test.py print statements causing pipe buffer deadlock (removed)


## Implementation Notes
- **Only spawn 2nd server during processing** — search only converts a single text string to an embedding vector, no benefit from a second server
- **Check free memory before spawning 2nd server** — each server process uses ~2GB RAM (or VRAM if GPU mode). Only start the second if there's enough headroom
- Keep single-server as the default; 2nd server is an optimization for batch processing only
