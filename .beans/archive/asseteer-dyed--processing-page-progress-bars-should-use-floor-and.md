---
# asseteer-dyed
title: Processing page progress bars should use floor % and smooth floating-point ratio
status: completed
type: task
priority: normal
created_at: 2026-03-25T12:17:43Z
updated_at: 2026-03-25T15:35:33Z
---

Two small improvements to progress display on the processing page:

1. The percentage label should use `Math.floor()` instead of rounding, so it never shows 100% until truly complete.
2. The progress bar width doesn't need to be clamped to whole-percent steps — it should use the raw floating-point ratio (e.g. `width: ${ratio * 100}%`) for smoother animation.

## Summary of Changes

- : Changed  to  for percentage label; added  derived for smooth floating-point bar width.
- : Same changes applied.
