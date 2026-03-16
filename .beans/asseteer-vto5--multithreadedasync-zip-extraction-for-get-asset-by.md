---
# asseteer-vto5
title: Multithreaded/async ZIP extraction for get_asset_bytes
status: todo
type: task
created_at: 2026-03-16T11:42:44Z
updated_at: 2026-03-16T11:42:44Z
---

Unzipping in get_asset_bytes should be properly async and potentially multithreaded for performance, especially relevant for cloud/canvas views that load many assets. Investigate faster zip crates.
