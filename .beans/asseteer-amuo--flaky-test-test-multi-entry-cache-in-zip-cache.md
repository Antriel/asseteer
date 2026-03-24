---
# asseteer-amuo
title: 'Flaky test: test_multi_entry_cache in zip_cache'
status: todo
type: bug
created_at: 2026-03-24T10:51:45Z
updated_at: 2026-03-24T10:51:45Z
---

Race condition between zip cache load and entry_count check. The test loads two nested ZIP entries then asserts entry_count() >= count_before + 2, but entries may get evicted between load and count. Fails intermittently.
