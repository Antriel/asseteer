---
# asseteer-cp49
title: Concurrent batch workers for CLAP processing
status: todo
type: task
priority: deferred
created_at: 2026-03-18T15:00:52Z
updated_at: 2026-03-18T15:00:52Z
parent: asseteer-526f
---

Send 2 concurrent batch requests (batch_size=8) to CLAP server. Benchmarks show 2.4x speedup over single requests (~55 files/sec for small SFX). Requires bumping CLAP concurrency limiter from 1 to 2.
