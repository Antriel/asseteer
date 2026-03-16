---
# asseteer-526f
title: CLAP processing performance
status: todo
type: epic
priority: high
created_at: 2026-03-16T09:38:05Z
updated_at: 2026-03-16T09:38:05Z
---

Processing a large SFX library (mostly ZIPs) takes an entire day. Three compounding bottlenecks identified: ZIP re-opening per file, no batch inference, and no I/O pipelining. This epic covers benchmarking to confirm assumptions and implementing targeted fixes.
