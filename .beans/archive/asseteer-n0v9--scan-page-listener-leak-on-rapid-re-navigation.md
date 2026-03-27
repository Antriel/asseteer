---
# asseteer-n0v9
title: Scan page listener leak on rapid re-navigation
status: completed
type: bug
priority: normal
created_at: 2026-03-16T09:19:43Z
updated_at: 2026-03-16T14:55:50Z
parent: asseteer-cfrp
---

In scan/+page.svelte, the scan-progress event listener is registered in startScan() (line 86) and stored in the unlisten variable. If the user navigates away during a scan and comes back, onMount (line 26) registers a second listener. If the user then triggers another scan, startScan registers a third listener without cleaning up the second. The onDestroy only cleans up whatever unlisten currently references. Each leaked listener will call handleProgress, causing state updates from stale contexts.

## Summary of Changes

Added cleanup of existing event listener at the start of `startScan()` in `src/routes/(app)/scan/+page.svelte`. Before registering a new `scan-progress` listener, the function now checks for and removes any previously registered listener. This prevents leaked listeners when the user navigates away during a scan and returns (where `onMount` registers a listener), then triggers another scan (where `startScan` would previously overwrite the reference without cleanup).
