---
# asseteer-swe5
title: START ALL parallel processing causes resource contention and RAM issues
status: todo
type: bug
priority: normal
created_at: 2026-03-26T14:50:28Z
updated_at: 2026-03-26T15:00:18Z
---

When using START ALL, processing categories run in parallel. Observed problem: CLAP started processing alongside audio, consumed a large amount of RAM, and appeared to starve audio processing — likely because the ZipCache was saturated by CLAP workloads. RAM usage grew well beyond ZipCache limits. Need to investigate: (1) whether running all categories in parallel is the right design, (2) why RAM isn't bounded by ZipCache limits during CLAP, and (3) whether CLAP and audio should share or compete for zip/cache resources.

## Notes from CLAP ZIP investigation (2026-03-26)

Current CLAP instrumentation shows throughput is dominated by Rust-side regular-ZIP prep, not Python inference:
- ~6-9s `prep_ms` per 16-file batch from a single outer ZIP
- ~0.3-0.4s Python upload/inference time for the same batch

This suggests CLAP is currently competing for regular ZIP extraction work in a way that is both slow and potentially hostile to running alongside other categories.

Planned follow-up in `asseteer-mmwn` is to adapt CLAP to the existing staged regular-ZIP dispatcher/shared-archive pattern already used for image/audio. That may help here in two ways:
- less duplicate ZIP parsing/extraction work across categories
- better-bounded buffering using the same pipeline/backpressure model

It also means this bean should be revisited after `asseteer-mmwn`, because shared staged dispatch may expose whether contention is fundamentally scheduling-related or caused by separate category resource models.
