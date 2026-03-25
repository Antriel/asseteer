---
# asseteer-3slc
title: Investigate better rate/ETA estimation during processing
status: todo
type: task
created_at: 2026-03-25T12:38:44Z
updated_at: 2026-03-25T12:38:44Z
---

Current rate and ETA display can be misleading — e.g. a long process that was fast early but slows toward the end will show a very short ETA for a long time, then suddenly jump to minutes remaining.

Investigate and implement better estimation:
- **Rate**: Use a rolling/windowed average (e.g. assets processed in the last N seconds) rather than total-average rate, so it reflects current throughput more accurately.
- **ETA**: Base ETA on the rolling rate, or a blend of rolling and total-average to avoid wild swings from momentary slowdowns.
- Consider standard approaches (EWMA — exponentially weighted moving average) as a candidate.
- Check whether the smoothing should happen on the backend before emitting progress events, or on the frontend when displaying.
