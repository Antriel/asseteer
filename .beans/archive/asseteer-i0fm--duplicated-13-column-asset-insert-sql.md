---
# asseteer-i0fm
title: Duplicated 13-column asset INSERT SQL
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:45:04Z
updated_at: 2026-03-21T08:33:15Z
parent: asseteer-c0lx
---

The same 13-column INSERT statement for assets appears in two places:
- `scan.rs:insert_asset_chunk` (lines 647-669) — batch insert during initial scan
- `rescan.rs:apply_rescan` (lines 389-411) — single insert during rescan apply

Both use identical SQL and binding order. If a column is added or renamed, both must be updated independently.

**Fix**: Extract the INSERT SQL as a constant, or better, create a shared `insert_asset(tx, asset)` function that both call.


## Summary of Changes

Extracted a shared `insert_asset_row(tx, asset, now)` function in `scan.rs`. Both `insert_asset_chunk` (scan) and `apply_rescan` (rescan) now delegate to it, eliminating the duplicated 13-column INSERT SQL.
