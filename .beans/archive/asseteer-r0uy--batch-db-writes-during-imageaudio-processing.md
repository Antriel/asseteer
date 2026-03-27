---
# asseteer-r0uy
title: Batch DB writes during image/audio processing
status: completed
type: task
priority: normal
created_at: 2026-03-23T08:17:48Z
updated_at: 2026-03-23T08:26:32Z
---

Replace per-item SQLite INSERTs during processing with a centralized batch writer.
Currently each of the 23 workers writes individually, causing ~500 transactions/sec
and heavy write lock contention. A shared DbBatchWriter collects results via channel
and flushes in batched transactions (64 items per tx, 100ms flush interval).

Expected impact: reduce SQLite transactions from ~500/sec to ~5-10/sec, lower system 
drive busy% from 40-80% to under 10%, and increase throughput from ~500 to 600-800+ items/sec.

## Tasks
- [x] Create db_writer.rs with DbBatchWriter (channel + writer task)
- [x] Add ProcessingOutput enum to processor.rs (CPU-only results)
- [x] Add process_asset_cpu() that returns data without DB writes
- [x] Modify work_queue.rs workers to use batch writer
- [x] Ensure tests pass with the new batched write path
- [x] Run cargo check

## Summary of Changes

Introduced a centralized `DbBatchWriter` that collects processing results from all workers via a tokio mpsc channel and flushes them in batched SQLite transactions (64 items per tx, 100ms flush interval).

- **New `db_writer.rs`**: `DbBatchWriter` with `send()` and `flush()` methods. Single background writer task with periodic + batch-size-triggered flushing. Proper sqlx transactions (`pool.begin()` / `commit()`) with fallback to individual writes on transaction failure.
- **New `process_asset_cpu()` / `process_image_cpu()` / `process_audio_cpu()`**: CPU-only processing functions that return `ProcessingOutput` enum instead of writing to DB directly.
- **Updated workers**: Image/Audio workers now use `process_asset_cpu()` and send results to the shared `DbBatchWriter`. Progress counters still update immediately. CLAP path unchanged (concurrency=1, no contention).
- **Flush on completion**: Both progress emitter and test completion monitor flush the batch writer before reporting completion.
- **All 66 tests pass**, no compiler warnings in production build.
