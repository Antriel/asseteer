---
# asseteer-z8dp
title: 'Processing progress: current filename disappears mid-processing'
status: in-progress
type: bug
priority: normal
created_at: 2026-03-21T12:07:18Z
updated_at: 2026-03-23T08:49:49Z
---


During audio processing (possibly also image/CLAP), the current filename line sometimes disappears entirely mid-processing after one of the periodic updates (~every 2s). Need to investigate why the filename becomes null/empty and fix it. If the behavior is expected, at minimum show the previous filename instead of nothing.

Additionally, after processing ends and shows "Completed: x", the count shown is much lower than the actual number of assets processed. Need to investigate why the completion count is under-reported.


## Investigation

### Root cause: small images (≤128px) create infinite pending loop

When `preGenerateThumbnails = true`:
1. Backend queries assets where `thumbnail_data IS NULL` — includes small images (≤128px)
2. Processor skips thumbnail generation for ≤128px images (by design — no point shrinking tiny images)
3. Writes `thumbnail_data = NULL` to DB
4. After completion, `refreshPendingCount()` re-counts them as pending
5. User sees 46450 "pending" assets → clicks Start → they reprocess in seconds → still pending → infinite loop

The `thumbnail_worker.rs:find_missing_thumbnails` already had the correct exclusion for ≤128px images, but the frontend query (`queries.ts`) and backend processing query (`process.rs`) did not.

### Fix applied
- `queries.ts:getPendingAssetCounts` — added `NOT (width <= 128 AND height <= 128)` exclusion
- `process.rs:start_processing` — same exclusion in the backend asset query
