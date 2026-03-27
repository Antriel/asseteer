---
# asseteer-e2df
title: Upgrade uv binary from v0.6.14 to v0.10.12
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:06:58Z
updated_at: 2026-03-20T11:07:17Z
parent: asseteer-5kja
---

Bump the pinned uv version and version-stamp the cache path so existing users auto-upgrade.

## Summary of Changes
- Bumped UV_VERSION from 0.6.14 to 0.10.12
- Version-stamped cache path: uv/\{version\}/uv.exe — ensures existing users auto-download the new binary
