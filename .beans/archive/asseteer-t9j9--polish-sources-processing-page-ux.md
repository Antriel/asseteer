---
# asseteer-t9j9
title: Polish Sources & Processing page UX
status: completed
type: task
priority: normal
created_at: 2026-03-26T11:37:02Z
updated_at: 2026-03-26T11:38:46Z
---

1. Constrain page width on wider displays (max-w + centered). 2. Remove redundant status badges from ProcessingCategoryCard (buttons already communicate state). 3. Simplify ClapProcessingCard badge to server-status only.


## Summary of Changes

- **Sources & Processing pages**: Added `max-w-3xl mx-auto` wrapper to constrain content width on wider displays
- **Sources page**: Added a subtle vertical divider before the destructive "Remove" button to visually separate it from safe actions
- **ProcessingCategoryCard**: Removed redundant status badge, added category icons (image/audio) matching ClapProcessingCard style, moved "completed in X" duration into the description line
- **ClapProcessingCard**: Simplified badge to only show server-level states (offline/starting), removed redundant processing-state badges (running/paused/idle etc.)
