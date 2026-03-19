---
# asseteer-xs88
title: 'Library image view: thumbnails stall after clicking sidebar folder filter'
status: todo
type: bug
created_at: 2026-03-19T11:40:43Z
updated_at: 2026-03-19T11:40:43Z
parent: asseteer-kvnt
---

In the Library images view, clicking a FOLDERS entry in the sidebar causes a view refresh. Already-loaded thumbnails start re-loading but never finish. Scrolling to new items works fine. No console errors. Likely a viewport-detection issue — the intersection observer or visibility tracking may not correctly re-trigger for items that were already in view before the filter changed.
