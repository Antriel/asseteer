---
# asseteer-usif
title: Repository stability/code-quality review
status: completed
type: task
priority: normal
created_at: 2026-02-14T07:30:19Z
updated_at: 2026-02-14T07:32:32Z
---

Review entire codebase for major duplication, code smells, and future stability risks. Create findings as beans grouped under a single epic.

## Summary of Changes

- Performed a repo-wide code review focused on major stability and maintainability risks.
- Created epic asseteer-bh0n to group findings.
- Added 5 child findings (4 bugs, 1 refactor task):
  - asseteer-i2fh stale queue items can be processed after stop/start
  - asseteer-j66e scan pipeline is blocking and all-in-memory
  - asseteer-six2 semantic search does full-table embedding scan each query
  - asseteer-tmo7 unchecked unwrap/expect can crash runtime paths
  - asseteer-e8eq duplicated processing card logic raises drift risk
