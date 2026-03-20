---
# asseteer-8p8a
title: 'Nested ZIP: add upfront memory availability guard before processing large inner ZIPs'
status: todo
type: task
priority: normal
created_at: 2026-03-19T16:21:56Z
updated_at: 2026-03-19T16:22:10Z
parent: asseteer-kvnt
---

The original nested-ZIP OOM mitigation bean also called for an upfront memory-availability check before queuing or loading large nested ZIP work. That guard has not been implemented yet. Add a pragmatic availability check or admission-control mechanism before starting expensive nested-ZIP processing.
