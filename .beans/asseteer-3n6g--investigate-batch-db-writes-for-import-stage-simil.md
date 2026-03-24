---
# asseteer-3n6g
title: Investigate batch DB writes for import stage (similar to asseteer-r0uy)
status: todo
type: task
created_at: 2026-03-23T15:43:25Z
updated_at: 2026-03-23T15:43:25Z
---

During a large import, high drive write rates were observed. The batch DB write optimization from asseteer-r0uy was applied to image/audio processing workers, but the import/scan stage may still be doing per-item writes. Investigate whether the same batching approach should be applied to the import pipeline to reduce write overhead and drive activity.
