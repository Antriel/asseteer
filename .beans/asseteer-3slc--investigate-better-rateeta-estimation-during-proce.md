---
# asseteer-3slc
title: Investigate better rate/ETA estimation during processing
status: completed
type: task
priority: normal
created_at: 2026-03-25T12:38:44Z
updated_at: 2026-03-26T11:25:44Z
---

Current rate and ETA display can be misleading — e.g. a long process that was fast early but slows toward the end will show a very short ETA for a long time, then suddenly jump to minutes remaining.

Investigate and implement better estimation:
- **Rate**: Use a rolling/windowed average (e.g. assets processed in the last N seconds) rather than total-average rate, so it reflects current throughput more accurately.
- **ETA**: Base ETA on the rolling rate, or a blend of rolling and total-average to avoid wild swings from momentary slowdowns.
- Consider standard approaches (EWMA — exponentially weighted moving average) as a candidate.
- Check whether the smoothing should happen on the backend before emitting progress events, or on the frontend when displaying.


## Implementation Plan

- [x] Add EWMA tracking fields to `CategoryState` (`prev_processed`, `ewma_rate`)
- [x] Rewrite `calculate_eta` to compute EWMA rate with α=0.1, blended with global average (0.7 EWMA + 0.3 global)
- [x] Fix pause bug (don't update EWMA while paused)
- [x] Update tests

## Summary of Changes

Replaced the naive global-average rate/ETA calculation with an EWMA-based `RateEstimator` struct. Uses α=0.1 (~40s effective window) blended 70/30 with global average for stability. The estimator lives locally in the progress emitter loop, so no changes to `CategoryState` were needed. Pause bug fixed — EWMA doesn't update while paused.
