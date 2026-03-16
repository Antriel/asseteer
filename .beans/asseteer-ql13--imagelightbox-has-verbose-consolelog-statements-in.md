---
# asseteer-ql13
title: ImageLightbox has verbose console.log statements in production
status: todo
type: task
priority: low
created_at: 2026-03-16T09:19:39Z
updated_at: 2026-03-16T09:19:39Z
parent: asseteer-cfrp
---

ImageLightbox.svelte has multiple console.log statements (lines 30, 36, 48, 52, 62, 83) that log for every asset change, blob creation, and cleanup. These are debug statements that pollute the console in production. Should be removed or gated behind a debug flag.
