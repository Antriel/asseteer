---
# asseteer-8yo6
title: Benchmark CLAP processing pipeline
status: in-progress
type: task
priority: high
created_at: 2026-03-16T09:38:19Z
updated_at: 2026-03-18T11:32:14Z
parent: asseteer-526f
---

Quick benchmark to measure where time is actually spent during CLAP processing, confirming assumptions before optimizing.

## What to measure
- **CLAP inference time** — How long does the Python server take per file? (isolate with direct HTTP calls)
- **ZIP extraction overhead** — How much time is spent opening/extracting from ZIPs vs reading regular files?
- **HTTP round-trip** — Network overhead per request
- **End-to-end per file** — Total wall time per asset through the full Rust→Python pipeline

## Approach
Temporary Tauri benchmark command that:
1. Takes a folder path (or ZIP path) and a file count limit
2. Processes N files with detailed timing breakdowns
3. Returns structured timing data (no DB writes needed)
4. Can be invoked from browser dev console via `invoke()`

Plus a simple Python script that hits the CLAP server directly to isolate inference time.

## Test cases
- [ ] 50-100 regular files from filesystem (baseline CLAP speed)
- [ ] 50-100 files from a single large ZIP (shows ZIP overhead)
- [ ] Compare to confirm where the bottleneck actually is

## Deliverable
Timing data that tells us: of the ~24h total processing time, roughly what % is ZIP I/O, what % is CLAP inference, what % is HTTP/other overhead.


## Results (2026-03-16)

Test data: Wild West Sound FX Pack Vol. 1 (220 WAV files, 41MB ZIP), tested 50 files.

| Scenario | Per file | Total (50) | % of time |
|----------|----------|------------|-----------|
| Filesystem → /embed/audio | 73ms | 3.7s | baseline |
| ZIP cached + upload | 77ms | 3.8s | +5% |
| ZIP reopen + upload | 78ms | 3.9s | +6% |

**Breakdown for ZIP reopen (current Rust behavior):**
- ZIP open: 1.3ms (2%)
- ZIP extract: 1.6ms (2%)
- Upload + inference: 74.8ms (96%)

**Conclusion:** CLAP inference is the bottleneck, not ZIP I/O. ZIP caching would save ~1%. Optimization effort should focus on batch inference and I/O pipelining.

Extrapolation: 10,000 files ≈ 12-13 min at this rate. A full-day processing run implies either a very large library (300K+ files) or other overhead in the Rust pipeline not captured here.


## Batch Results (2026-03-16)

| Mode | Per file | Speedup | Extrapolation (283K files) |
|------|----------|---------|---------------------------|
| Single file | 74ms | 1.0x | 5.8 hours |
| Batch size=4 | 51ms | 1.4x | 4.0 hours |
| Batch size=8 | 51ms | 1.5x | 4.0 hours |
| Batch size=16 | 50ms | 1.5x | 4.0 hours |

Batching plateaus at ~1.5x. Reason: model forward pass is only 66% of in-process time.

### In-process breakdown (no HTTP)
- librosa.load(): 1.8ms (3%) — audio decoding, trivial for WAV
- processor() (mel spectrogram): 15.6ms (30%) — runs per-file even in batch
- model inference: 34.2ms (66%) — the part batching amortizes
- Total in-process: 51.6ms
- HTTP overhead: ~22ms (brings it to 74ms via HTTP)

### Remaining optimization vectors
1. **Rust concurrency=2-3**: overlap 22ms HTTP wait with next file's ZIP extraction (free ~22ms)
2. **Parallel librosa/processor in Python**: ThreadPoolExecutor for audio loading while model runs
3. **Both combined**: could bring per-file down to ~35-40ms → ~3 hours for full library


## Multi-Process Scaling Results (2026-03-16)

| Config | files/sec | Speedup | 283K files |
|--------|-----------|---------|------------|
| 1 server, sequential | 14.9 | 1.0x | 5.3h |
| 1 server, concurrent=2 | 23.5 | 1.6x | 3.4h |
| 1 server, batch=8 | 21.2 | 1.4x | 3.7h |
| 2 servers, concurrent=4 | 33.3 | 2.2x | 2.4h |
| 4 servers, concurrent=4 | 33.8 | 2.3x | 2.3h |
| 4 servers, batch=8, concurrent=4 | 30.1 | 2.0x | 2.6h |

**Key findings:**
- 2 server processes is the sweet spot (2.2x, ~4GB RAM). 4 servers barely adds anything.
- Simple concurrency (multiple single requests) beats batching in multi-server setup.
- Batching slightly hurts in multi-server because it adds per-request latency.
- CPU/memory bandwidth is the bottleneck at 2+ servers, not the GIL.

**Recommended production config:** 2 server processes + 4 concurrent Rust workers.

**Bugs found during benchmarking:**
- `async def` endpoints blocked the event loop (fixed: changed to sync `def`)
- `clap_test.py` verbose prints filled subprocess pipe buffer causing deadlock (fixed: removed prints)


## HTTP Overhead Benchmark (2026-03-18)

Added a /noop endpoint and measured pure HTTP round-trip (200 requests):

| Mode | Mean | Median |
|------|------|--------|
| New connection each | 0.63ms | 0.61ms |
| Keep-alive | 0.26ms | 0.23ms |

**Conclusion:** The 22ms "HTTP overhead" from earlier benchmarks is actually server contention (waiting for the previous inference to finish), not transport cost. HTTP on localhost is essentially free (<1ms). No need for WebSocket, Unix sockets, or any transport-layer optimization.
