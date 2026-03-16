---
# asseteer-260m
title: AudioPlayer progress bar seek uses keyboard event as MouseEvent
status: todo
type: bug
created_at: 2026-03-16T09:19:32Z
updated_at: 2026-03-16T09:19:32Z
parent: asseteer-cfrp
---

In AudioPlayer.svelte:303, the progress bar's onkeydown handler casts the KeyboardEvent to MouseEvent via 'seek(e as any)'. The seek function (line 210-216) reads e.clientX to calculate position. A KeyboardEvent has no meaningful clientX (it's 0), so pressing Enter/Space on the progress bar always seeks to time 0. This is a broken accessibility handler — keyboard seek should either use a different calculation or be disabled.
