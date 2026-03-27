---
# asseteer-u1mx
title: Add batch audio embedding endpoint to CLAP server
status: scrapped
type: feature
priority: low
created_at: 2026-03-16T09:39:02Z
updated_at: 2026-03-20T09:57:32Z
parent: asseteer-526f
blocked_by:
    - asseteer-8yo6
---

Batch audio embedding endpoint for the CLAP server.

**DEPRIORITIZED**: Benchmarks showed batching provides 1.4-1.5x in single-server mode, but actually hurts throughput in multi-server setups (which is the recommended config). Multi-process + concurrency (2 servers, 4 concurrent) achieves 2.2x without batching complexity.

The batch endpoints are already implemented in the server (during benchmarking), but integrating them into the Rust client is not worth the added complexity. Simple concurrent single-file requests to multiple server processes is both simpler and faster.

Revisit only if moving to GPU inference where batch size significantly impacts throughput.

## Reasons for Scrapping
Parent epic scrapped — not worth extra complexity.
