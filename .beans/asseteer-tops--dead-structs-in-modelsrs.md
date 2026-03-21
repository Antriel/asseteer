---
# asseteer-tops
title: Dead structs in models.rs
status: todo
type: task
created_at: 2026-03-20T11:45:16Z
updated_at: 2026-03-20T11:45:16Z
parent: asseteer-c0lx
---

Several structs in `models.rs` appear unused by application code:

1. **`ScanProgress`** (lines 68-75) — has fields `session_id`, `total_files`, `processed_files`, `current_file`, `status`. The actual scan progress struct used is `commands::scan::ScanProgress` with completely different fields (`phase`, `files_found`, `files_inserted`, etc.). This is a name collision with dead code.

2. **`ScanSession`** (lines 56-66) — scan sessions are managed via raw SQL queries in `scan.rs`, never using this struct for deserialization.

3. **`PendingCount`** (lines 127-131) — has fields `images`, `audio`, `total` but appears unused. Pending counts are fetched as simple `(i64,)` tuples.

**Fix**: Verify with `cargo check` warnings or grep for usage, then remove dead structs.
