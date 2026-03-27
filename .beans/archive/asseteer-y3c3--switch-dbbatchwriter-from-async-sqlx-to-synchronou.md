---
# asseteer-y3c3
title: Switch DbBatchWriter from async sqlx to synchronous rusqlite
status: completed
type: task
priority: normal
created_at: 2026-03-25T06:52:56Z
updated_at: 2026-03-25T06:59:11Z
---

Unify the processing write path with the scan bulk-insert pattern: use a rusqlite connection on spawn_blocking with wal_autocheckpoint=0, crossbeam channel, and prepare_cached statements.

## Summary of Changes

Rewrote `DbBatchWriter` to use synchronous rusqlite on `spawn_blocking`, matching the scan bulk-insert pattern:

- **`db_writer.rs`**: Replaced async sqlx writer task with synchronous rusqlite writer:
  - `tokio::sync::mpsc` → `crossbeam::channel` (enables `recv_timeout` for periodic flush)
  - `tokio::spawn(writer_task)` → `tokio::task::spawn_blocking(writer_task_sync)`
  - Async sqlx queries → `rusqlite::Connection::prepare_cached()` + `execute(params![...])`
  - `PRAGMA wal_autocheckpoint=0` on the writer connection (no mid-processing checkpoint I/O)
  - Same batching (64 items), flush interval (100ms), and fallback-to-individual-writes behavior
  - Tests switched from in-memory DB to file-backed via `tempfile::tempdir()`
- **`work_queue.rs`**: `ensure_db_writer` and `start`/`start_for_test` now take `db_path: &str` instead of using the SqlitePool for the writer
- **`process.rs`**: Updated both `start_processing` and `retry_failed_assets` to pass `&state.db_path`

All 66 tests pass.
