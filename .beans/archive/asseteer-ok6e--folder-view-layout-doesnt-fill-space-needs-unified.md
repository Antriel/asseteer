---
# asseteer-ok6e
title: 'Folder view layout: doesn''t fill space, needs unified layout with Processing tab'
status: completed
type: bug
priority: normal
created_at: 2026-03-19T11:40:37Z
updated_at: 2026-03-19T11:56:50Z
parent: asseteer-kvnt
---

The /folders route doesn't use the full available width the way the Processing tab does. Should unify the max-width/padding/centering CSS so both tabs feel consistent on wide displays.

## Summary of Changes

Removed `max-w-3xl` inner wrapper div and aligned the outer container with the Processing tab: `flex flex-col h-full overflow-auto p-6`. Content now fills the available width consistently on wide displays.
