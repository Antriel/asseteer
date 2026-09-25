---
# asseteer-zw0i
title: Audio player progress bar not visible in dark mode
status: completed
type: bug
priority: normal
created_at: 2026-04-02T13:09:41Z
updated_at: 2026-09-25T09:22:51Z
---

The progress bar in the audio player is not visible when using dark mode.

## Summary of Changes
Cause: the track used `bg-default`, a class that doesn't exist, so the unfilled track was transparent (only the blue fill ever showed). Added a `--color-track` token (+ `.bg-track`) for both themes. The scrubber was also redone as part of the transport strip (asseteer-74pa): 4px track that grows on hover, a thumb dot, drag-to-scrub via pointer capture, and `timeupdate` sync so seeking while paused moves the playhead.
