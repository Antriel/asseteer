---
# asseteer-wg6t
title: Settings page cards don't use full horizontal width like Processing page
status: completed
type: bug
priority: normal
created_at: 2026-03-25T12:04:31Z
updated_at: 2026-03-25T15:38:16Z
---

The settings page CSS/UI doesn't stretch cards to use the full available horizontal space, unlike the processing page which does this correctly.

## Summary of Changes

Removed the  inner wrapper div and aligned the outer container to match the processing page pattern (). Cards now stretch to full available width.
