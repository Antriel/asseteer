---
# asseteer-q6z5
title: Speed up CLAP processing throughput
status: completed
type: feature
priority: high
created_at: 2026-03-22T08:31:22Z
updated_at: 2026-03-23T12:20:02Z
parent: asseteer-526f
---


CLAP processing is ~100x slower than metadata processing. The main bottleneck is low GPU/CPU utilization due to serial request handling and idle time between batches. This bean tracks improvements to CLAP throughput without requiring a second server process.

## Root Cause Analysis

The CLAP pipeline is:
1. Rust worker prepares batch (load from ZIP cache / filesystem) — GPU idle
2. HTTP request to Python server — GPU idle
3. Server loads files sequentially with `librosa.load()` (CPU) — GPU idle
4. Single `model.get_audio_features()` forward pass (GPU) — GPU active
5. HTTP response back — GPU idle
6. Repeat (concurrency=1, so no overlap)

GPU spends most of its time waiting for the next batch. The Python server (FastAPI + uvicorn, 1 worker, sync handlers) processes requests sequentially — even if Rust sent 2 concurrent requests, they'd queue.

## Improvement Options (to benchmark)

### Option A: Increase batch size (simplest)
- Current: `CLAP_BATCH_SIZE = 8` (work_queue.rs:17)
- Try 16, 32, 64 — more files per forward pass = GPU stays busy longer
- **Concern: system portability** — larger batches need more VRAM (GPU) or RAM (CPU). A batch size tuned for one system may OOM on another. Options:
  - Adaptive: start at 8, ramp up if requests succeed, back off on OOM/timeout
  - Query VRAM/RAM at startup and set batch size accordingly
  - Cap conservatively (e.g., 16) and let users override in settings
  - Benchmark on CPU-only vs GPU to find safe defaults for each

### Option B: Make Python server handle concurrent requests
- Convert sync `/embed/audio/batch` handler to use `run_in_executor` (the upload endpoint already does this)
- Bump Rust-side CLAP concurrency from 1 to 2
- Result: while batch 1 is in GPU forward pass, batch 2 preprocesses audio on CPU
- True pipelining — CPU and GPU stay busy simultaneously
- **Concern**: librosa preprocessing in thread pool + inference on main thread must not contend. PyTorch GIL release during CUDA forward pass should allow this.

### Option C: Server-side pipeline (more invasive)
- Restructure Python server to continuously prefetch and preprocess the next batch in a background thread while current batch is inferencing
- Most efficient but biggest change to server architecture

### Option D: Second server process (deferred)
- Each CLAP model ~600MB-2GB depending on CPU/GPU mode
- Hard to determine safely if memory is available across different systems
- Deferred — try A+B first

## Previous Benchmarks (from asseteer-5m6z / asseteer-cp49)
- 1 server, 1 concurrent: 14.9 files/sec
- 2 concurrent batches to 1 server: ~55 files/sec (2.4x — likely measuring Rust-side overlap)
- 2 servers, 4 concurrent: 33.3 files/sec (2.2x over baseline)

## Key Files
- `src-tauri/src/task_system/work_queue.rs` — CLAP_BATCH_SIZE, concurrency semaphore
- `clap-server/clap_server.py` — batch endpoints, `_batch_encode()`, `run_in_executor`
- `src-tauri/src/clap/client.rs` — HTTP client, timeouts
- `src-tauri/src/task_system/processor.rs` — `process_clap_embedding_batch()`

## Implementation Checklist
- [x] Benchmark current throughput as baseline (files/sec, GPU utilization)
- [x] Benchmark increased batch sizes (16, 32) on GPU and CPU
- [x] Determine batch size strategy (adaptive vs conservative default)
- [x] Make server batch endpoint concurrent via `run_in_executor`
- [x] Bump Rust CLAP concurrency to 2
- [x] Benchmark combined improvements
- [x] Adjust HTTP timeouts if batch size increases (currently 120s) — 120s still sufficient at batch=16

## Summary of Changes

- **Option B (concurrent requests)**: Made `/embed/audio/batch` async with `run_in_executor`, bumped Rust CLAP concurrency from 1 to 2. Allows CPU preprocessing of batch N+1 to overlap with GPU inference of batch N.
- **Option A (batch size)**: Bumped `CLAP_BATCH_SIZE` from 8 to 16.
- **Duration cap**: Added `duration=max_length_s` (~10s) to all `librosa.load()` calls — avoids loading/resampling entire large files when CLAP only uses a fixed window. File-path endpoint takes the middle segment; byte-buffer backends trim to middle after decode.
- Combined result: significant throughput improvement on both small and large file packs.
