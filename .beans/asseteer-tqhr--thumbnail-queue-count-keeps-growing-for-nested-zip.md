---
# asseteer-tqhr
title: Thumbnail queue count keeps growing for nested-ZIP images without scrolling
status: todo
type: bug
created_at: 2026-03-21T12:07:25Z
updated_at: 2026-03-21T12:07:25Z
---

After importing a mostly zip-based bundle and opening the images view, each zip contains one large image file (nested zips). Thumbnails show as loading (expected, since unpacking takes time). However, the queued thumbnail count in the status bar keeps increasing even without any scrolling. Also suspect the virtual list padding rows (how far beyond visible area we queue thumbnails) may be larger than the intended 2 rows — worth double-checking.
