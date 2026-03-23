---
# asseteer-526f
title: CLAP processing performance
status: todo
type: epic
priority: high
created_at: 2026-03-16T09:38:05Z
updated_at: 2026-03-22T08:32:13Z
---

Processing a large SFX library (mostly ZIPs) takes an entire day. Three compounding bottlenecks identified: ZIP re-opening per file, no batch inference, and no I/O pipelining. This epic covers benchmarking to confirm assumptions and implementing targeted fixes.


## Quick Wins Implemented (2026-03-18)
- [x] Batch CLAP inference (batch_size=8): ~1.9x speedup for small files
- [x] Inner ZIP caching: ~28% speedup for nested ZIPs
- [x] Fixed broken batch endpoint in CLAP server

## Summary of Changes
Batch inference (~1.9x speedup), inner ZIP caching (~28% speedup), and fixed batch endpoint were implemented. Remaining optimizations (multi-server, I/O pipelining, ONNX, faster decoders) were scrapped as diminishing returns for the complexity cost.
