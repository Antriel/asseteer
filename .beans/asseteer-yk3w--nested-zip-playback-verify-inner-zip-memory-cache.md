---
# asseteer-yk3w
title: 'Nested ZIP playback: verify inner-ZIP memory cache is used for audio playback'
status: todo
type: task
created_at: 2026-03-19T11:41:45Z
updated_at: 2026-03-19T11:41:45Z
parent: asseteer-kvnt
---

Playing audio from nested ZIPs is slow to start. asseteer-mxsj implemented an inner-ZIP memory cache (load_asset_bytes_cached) for CLAP processing, but it's unclear if this fast path is also used during playback (get_asset_bytes command). Need to trace the playback code path and confirm it hits the cache. If not, wire it up.
