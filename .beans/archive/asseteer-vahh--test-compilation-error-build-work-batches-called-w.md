---
# asseteer-vahh
title: Tests fail to compile (15 errors)
status: completed
type: bug
priority: critical
created_at: 2026-03-20T11:45:10Z
updated_at: 2026-03-20T11:57:49Z
parent: asseteer-c0lx
---

`build_work_batches` requires 4 parameters (category, generation, assets, pre_generate_thumbnails) but is called with only 3 in test code:

- `work_queue.rs:660` in `start_for_test`: `Self::build_work_batches(category, generation, assets)` — missing `pre_generate_thumbnails`
- `work_queue.rs:863`: `WorkQueue::build_work_batches(ProcessingCategory::Audio, 7, assets)` — missing 4th arg
- `work_queue.rs:883`: `WorkQueue::build_work_batches(ProcessingCategory::Audio, 3, assets)` — missing 4th arg

This means `cargo test` will fail to compile. The 4th parameter (`pre_generate_thumbnails: bool`) was likely added after the tests were written and the test calls were not updated.

**Fix**: Add the missing `false` argument to all 3 call sites.


---

**UPDATE**: `cargo check --tests` reveals 15 compilation errors total:
- 3x `build_work_batches` called with 3 args instead of 4 (missing `pre_generate_thumbnails`)
- 12x `Asset` struct constructors missing `zip_compression` field (added to model but not to test helpers)

The root cause for the 12 missing-field errors is `test_helpers.rs:make_asset()` — it doesn't include `zip_compression`. Fixing that one helper + adding the field to the handful of inline `Asset { ... }` constructors in test files will resolve all 12.

Files affected:
- `test_helpers.rs` (1 fix covers most)
- `utils.rs` test Asset constructors
- `zip_cache.rs` test Asset constructors
- `processor.rs` test Asset constructor
- `work_queue.rs` build_work_batches calls

## Summary of Changes

Fixed all 15 compilation errors in test code:

- **`test_helpers.rs`**: Added `zip_compression: None` to `make_asset()` — this one fix propagated to most test helpers that call it.
- **`utils.rs`**: Added `zip_compression: None` to 7 inline `Asset { ... }` struct literals in tests.
- **`zip_cache.rs`**: Added `zip_compression: None` to 3 inline `Asset { ... }` struct literals in tests (`make_nested_zip_asset`, `test_non_zip_asset_bypasses_cache`, `test_simple_zip_asset_bypasses_cache`).
- **`processor.rs`**: Added `zip_compression: None` to 1 inline `Asset { ... }` struct literal in `test_generate_thumbnail_produces_webp`.
- **`work_queue.rs`**: Added missing `false` (4th `pre_generate_thumbnails` arg) to 3 `build_work_batches` call sites (`start_for_test` line 660, and two test functions at lines 863 and 883).

`cargo check --tests` now passes cleanly.
