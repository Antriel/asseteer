---
# asseteer-mmwn
title: CLAP regular-ZIP staged pipeline and shared extraction planning
status: in-progress
type: task
priority: high
created_at: 2026-03-26T15:00:01Z
updated_at: 2026-03-26T15:00:01Z
---

## Problem

CLAP throughput is dominated by Rust-side batch preparation, not Python decode/inference. Instrumentation shows regular ZIP-backed CLAP batches spend ~6-9 seconds in prep for 16 assets from a single outer ZIP, while the Python server processes the uploaded batch in ~0.3-0.4 seconds.

Observed instrumentation:
- `distinct_zip_files=1` for the slow batches, so locality is already good at the outer-ZIP level
- `prep_ms` is consistently 6000-9000 ms
- `zip_request_ms` is consistently 300-400 ms
- Python logs confirm `/embed/audio/batch/upload` completes quickly once the request arrives

This means the current CLAP path is still doing expensive ZIP extraction work synchronously in the worker before every upload, and 2 CLAP workers are not enough to keep the GPU fed because both spend most of their time in ZIP prep.

## Why this matters

We already solved the same shape of problem for image/audio processing in `work_queue.rs`: regular ZIP groups can be staged through a shared open archive with bounded prefetch (`PIPELINE_DEPTH`) and `bulk_load_from_archive()`. CLAP currently has its own batching path and does not reuse that staged regular-ZIP dispatcher pattern.

Keeping CLAP separate here increases maintenance cost and makes queueing/cancellation/resource behavior drift from the other processing categories.

## Proposed direction

Adapt the existing staged regular-ZIP pipeline for CLAP rather than inventing a CLAP-only extractor:

1. Review the image/audio staged-dispatch path and identify the reusable pieces:
   - ZIP-group planning for regular archives
   - shared open-archive extraction via `bulk_load_from_archive()`
   - bounded pipeline buffering / backpressure
   - worker handoff using `preloaded_bytes`
2. Refactor shared pieces if needed so CLAP can use the same batch-group dispatch model with minimal category-specific branching.
3. Change CLAP planning so regular ZIP groups can go through staged dispatch, while nested ZIP handling can stay on `zip_cache`.
4. Keep instrumentation during rollout so we can verify that CLAP `prep_ms` drops sharply and GPU idle gaps shrink.
5. Ensure stop/cancel semantics remain correct when batches are pre-extracted or buffered.

## Design constraints

- Prefer sharing the existing regular-ZIP staged-dispatch machinery instead of duplicating it
- Keep nested ZIP behavior separate where necessary; the existing `zip_cache` model is still appropriate there
- Avoid an unbounded preload strategy; reuse the existing bounded pipeline approach
- Do not move audio decode into Rust unless ZIP-side staging proves insufficient

## Related beans

- Builds directly on `asseteer-50a1` (regular ZIP staged dispatch + shared archive)
- Informed by `asseteer-ctrc` (root cause for non-nested ZIP archive parsing overhead)
- Revisits assumptions from `asseteer-q6z5` (CLAP concurrency/batch sizing helped, but prep is still the dominant bottleneck)
- Likely interacts with `asseteer-t9m5` (queued work cancellation) and `asseteer-swe5` (resource contention across categories)

## Tasks
- [ ] Review regular ZIP staged dispatch in image/audio and identify what should be shared with CLAP
- [ ] Design a shared batch-group extraction/upload flow that CLAP can reuse without large code duplication
- [ ] Implement lightweight CLAP planning changes so regular ZIP batches use staged dispatch with preloaded bytes
- [ ] Verify with instrumentation that CLAP prep time drops materially and Python remains a minor fraction of batch time
- [ ] Review stop/cancel and cross-category resource behavior after the pipeline change
