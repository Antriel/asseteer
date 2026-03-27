---
# asseteer-yk3w
title: 'Nested ZIP playback: verify inner-ZIP memory cache is used for audio playback'
status: completed
type: task
priority: normal
created_at: 2026-03-19T11:41:45Z
updated_at: 2026-03-20T07:33:04Z
parent: asseteer-kvnt
---

Playing audio from nested ZIPs is slow to start. asseteer-mxsj implemented an inner-ZIP memory cache (load_asset_bytes_cached) for CLAP processing, but it's unclear if this fast path is also used during playback (get_asset_bytes command). Need to trace the playback code path and confirm it hits the cache. If not, wire it up.

## Summary of Changes

`get_asset_bytes` (the playback command) was calling `utils::load_asset_bytes` directly, bypassing the nested-ZIP memory cache entirely. Changed it to use `zip_cache::load_asset_bytes_cached` — the same path used by the CLAP processor. For simple ZIPs and plain files it falls through unchanged; for nested ZIPs it now hits the single-slot `Arc<Vec<u8>>` cache, avoiding re-decompression of the inner ZIP on each playback start.
