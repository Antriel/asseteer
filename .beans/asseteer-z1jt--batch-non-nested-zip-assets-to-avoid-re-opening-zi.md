---
# asseteer-z1jt
title: Batch non-nested ZIP assets to avoid re-opening ZIP per entry
status: completed
type: task
priority: normal
created_at: 2026-03-25T06:34:00Z
updated_at: 2026-03-25T06:36:51Z
---

Non-nested ZIP assets are each processed individually, re-opening and re-parsing the ZIP central directory for every single entry. Group them by ZIP path (batch size ~16) and bulk-extract bytes in one pass.

## Summary of Changes

Three files changed to batch non-nested ZIP assets and bulk-extract their bytes in a single ZIP open:

- **`utils.rs`**: Added `bulk_load_from_zip()` — opens a ZIP once, extracts multiple entries in one pass, returns `HashMap<asset_id, Result<Vec<u8>, String>>`
- **`processor.rs`**: Added `process_asset_cpu_with_bytes()` + `process_image_cpu_with_bytes()` + `process_audio_cpu_with_bytes()` — skip the file loading step, process from pre-loaded bytes
- **`work_queue.rs`**: `build_batch_plan()` now groups non-nested ZIP assets by ZIP path (batch size 16). Worker loop bulk-extracts bytes for same-ZIP batches before processing each asset.

All 66 tests pass.
