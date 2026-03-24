---
# asseteer-p53o
title: Use synchronous rusqlite for bulk asset insertion during scan
status: todo
type: task
priority: normal
created_at: 2026-03-24T11:00:08Z
updated_at: 2026-03-24T11:00:08Z
---

The scan insertion loop currently does 940k individual `sqlx::query().bind(×13).execute()` calls through the async runtime. The per-statement async overhead (tokio poll cycles, connection pool dispatch, statement cache lookup) is the primary bottleneck — NVME at ~1% utilization (20-30 MB/s of 3000+ MB/s capacity), CPU appears idle.

## Approach

Bypass sqlx's async layer for the bulk insert hot path. Use a synchronous `rusqlite::Connection` on `spawn_blocking` with a tight prepared-statement-reuse loop:

```
spawn_blocking:
  open rusqlite::Connection to same DB file
  PRAGMA wal_autocheckpoint=0
  BEGIN
  let stmt = conn.prepare("INSERT OR IGNORE INTO assets (...) VALUES (?, ?, ...)")
  for each asset:
    stmt.execute(params![...])
    stmt.reset()
  COMMIT
```

This eliminates per-row async overhead and matches SQLite's canonical bulk insert pattern (prepare once, rebind+step+reset in loop, single transaction). Should achieve 500k+ rows/sec based on SQLite benchmarks.

## Implementation Notes

- `rusqlite` is already a transitive dependency via `sqlx-sqlite`, but may need a direct `Cargo.toml` dep
- Discovery already streams chunks via mpsc — the receiving side changes from async sqlx to sync rusqlite on spawn_blocking
- The rusqlite connection needs `busy_timeout(30s)` and `journal_mode=WAL` to coexist with the sqlx pool
- Keep sqlx pool for everything else (queries, updates, processing writes) — this is only for the scan bulk insert path
- Channel capacity (32 chunks × 1000 assets) provides buffering between async/sync boundary
- FTS batched population still uses sqlx (it's INSERT...SELECT, only ~19 statements total)
