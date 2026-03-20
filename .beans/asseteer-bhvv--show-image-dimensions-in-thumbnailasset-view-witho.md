---
# asseteer-bhvv
title: Show image dimensions in thumbnail/asset view without reload
status: todo
type: bug
created_at: 2026-03-20T08:26:28Z
updated_at: 2026-03-20T08:26:28Z
parent: asseteer-kvnt
---

When thumbnails lazy-load, they trigger processing which produces width/height data. However this data isn't shown in the UI unless the image is unloaded and reloaded. Investigate whether the dimensions are available at thumbnail load time and surface them in the UI without requiring a reload. Should be an easy fix if the data is already in the DB after thumbnail generation.
