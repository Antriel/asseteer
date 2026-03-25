---
# asseteer-40vq
title: Audio processing start has no feedback before progress begins
status: todo
type: bug
created_at: 2026-03-24T12:32:22Z
updated_at: 2026-03-24T12:32:22Z
---

When clicking to start audio processing, there's no immediate feedback until progress starts (can take ~1 second). Also, the 'Processing: <filepath>' line sometimes disappears (shows null) — should show last entry instead of hiding the line to avoid UI jitter.
