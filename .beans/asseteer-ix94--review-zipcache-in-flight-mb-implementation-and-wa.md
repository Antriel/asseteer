---
# asseteer-ix94
title: Review ZipCache in_flight_mb implementation and WARN slow LOAD warnings
status: todo
type: task
created_at: 2026-03-23T15:43:25Z
updated_at: 2026-03-23T15:43:25Z
---

During large imports, many 'ZipCache] WARN slow LOAD' messages appear, e.g.: size_mb=1004.8 load_ms=18596 entries=8 cached_mb=1005 in_flight_mb=6542. We recently added in_flight_mb tracking and should verify: (1) the in_flight_mb accounting is correct, (2) the warnings are expected/acceptable at this scale, (3) whether the slow loads indicate a real problem or are just noisy.
