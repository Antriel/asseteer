---
# asseteer-j1db
title: Duplicated processing error INSERT in work_queue.rs
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:45:01Z
updated_at: 2026-03-21T08:34:34Z
parent: asseteer-c0lx
---

The exact same error-recording SQL block appears twice in `work_queue.rs`:
- Lines 326-335 (CLAP batch path)
- Lines 374-384 (Image/Audio path)

Both do:
```rust
let now = unix_now();
let _ = sqlx::query(
    "INSERT INTO processing_errors (asset_id, category, error_message, occurred_at, retry_count)
     VALUES (?, ?, ?, ?, 0)"
)
.bind(result.asset_id)
.bind(category.as_str())
.bind(error_msg)
.bind(now)
.execute(&db)
.await;
```

**Fix**: Extract to a `record_processing_error(asset_id, category, error_msg, db)` helper function.

## Summary of Changes

Extracted duplicated `processing_errors` INSERT into `record_processing_error(asset_id, category, error_msg, db)` async helper (added after `unix_now()`). Replaced both call sites (CLAP batch path and Image/Audio path) with a single call.
