---
# asseteer-cfrp
title: 'Code review: bugs, performance, and UI issues (March 2026)'
status: completed
type: epic
priority: normal
created_at: 2026-03-16T09:18:37Z
updated_at: 2026-03-16T15:01:26Z
---

Findings from a focused code review of the frontend SvelteKit codebase. Covers bugs, performance concerns, and UI problems.


## Findings Summary

### Bugs (8)

| ID | Title | Priority |
|----|-------|----------|
| asseteer-fkqr | ImageGrid columnCount doesn't update on window resize | normal |
| asseteer-wih9 | totalMatchingCount always equals result.length | high |
| asseteer-laao | Lightbox navigation index desyncs from filtered list | normal |
| asseteer-qxu6 | ImageThumbnail blob URLs leak on re-key | normal |
| asseteer-wkyq | AssetList table virtual scrolling broken with spacers | normal |
| asseteer-ppee | Tab switch doesn't reload assets for new tab | normal |
| asseteer-260m | AudioPlayer progress bar keyboard seek always goes to 0 | normal |
| asseteer-n0v9 | Scan page listener leak on rapid re-navigation | normal |

### Lower severity / quality (4)

| ID | Title | Priority |
|----|-------|----------|
| asseteer-rvo8 | AudioPlayer volume not synced on asset change | low |
| asseteer-udc8 | DurationFilter state persists across tab switches | normal |
| asseteer-w3gl | Hardcoded row heights drift from actual layout | low |
| asseteer-ql13 | Verbose console.log in ImageLightbox | low |

### Already tracked (existing epic asseteer-bh0n)

These were found in the prior review and are NOT duplicated here:
- Processing queue stale work (asseteer-i2fh)
- Scan pipeline blocking/in-memory (asseteer-j66e)
- Semantic search full-table scan (asseteer-six2)
- Unchecked unwrap/expect crashes (asseteer-tmo7)
- Duplicate processing card logic (asseteer-e8eq)

## Summary of Changes

All 12 findings from the March 2026 code review have been addressed:
- 8 bugs fixed (resize, virtual scrolling, lightbox navigation, blob URL leaks, tab switching, keyboard seek, listener leaks, totalMatchingCount)
- 3 lower-severity issues resolved (volume sync, duration filter visibility, console.log cleanup)
- 1 hardcoded-heights issue fixed with dynamic measurement
- 1 bug scrapped (tab switch reload — asseteer-ppee)
