---
# asseteer-amuo
title: 'Flaky test: test_multi_entry_cache in zip_cache'
status: completed
type: bug
priority: normal
created_at: 2026-03-24T10:51:45Z
updated_at: 2026-03-24T11:04:43Z
---

Race condition between zip cache load and entry_count check. The test loads two nested ZIP entries then asserts entry_count() >= count_before + 2, but entries may get evicted between load and count. Fails intermittently.

## Summary of Changes

Removed the flaky global `entry_count()` / `cached_bytes()` assertions that depended on shared global cache state (other parallel tests call `clear()`). Replaced with reload assertions that verify cache hits return identical data — proving both entries were cached without depending on global counts.
