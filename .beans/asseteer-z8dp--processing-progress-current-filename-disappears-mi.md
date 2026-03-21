---
# asseteer-z8dp
title: 'Processing progress: current filename disappears mid-processing'
status: todo
type: bug
created_at: 2026-03-21T12:07:18Z
updated_at: 2026-03-21T12:07:18Z
---

During audio processing (possibly also image/CLAP), the current filename line sometimes disappears entirely mid-processing after one of the periodic updates (~every 2s). Need to investigate why the filename becomes null/empty and fix it. If the behavior is expected, at minimum show the previous filename instead of nothing.
