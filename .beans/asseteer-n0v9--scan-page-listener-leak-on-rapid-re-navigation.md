---
# asseteer-n0v9
title: Scan page listener leak on rapid re-navigation
status: todo
type: bug
created_at: 2026-03-16T09:19:43Z
updated_at: 2026-03-16T09:19:43Z
parent: asseteer-cfrp
---

In scan/+page.svelte, the scan-progress event listener is registered in startScan() (line 86) and stored in the unlisten variable. If the user navigates away during a scan and comes back, onMount (line 26) registers a second listener. If the user then triggers another scan, startScan registers a third listener without cleaning up the second. The onDestroy only cleans up whatever unlisten currently references. Each leaked listener will call handleProgress, causing state updates from stale contexts.
