---
# asseteer-laao
title: Lightbox navigation index desyncs from filtered asset list
status: todo
type: bug
priority: normal
created_at: 2026-03-16T09:18:56Z
updated_at: 2026-03-16T09:22:36Z
parent: asseteer-cfrp
---

In view.svelte.ts:39-51, nextImage/prevImage use this.lightboxIndex to index into the assets array passed as a parameter. But lightboxIndex is set from ImageGrid's startIndex + idx (line 106 of ImageGrid.svelte), which is the index into the full assets array. If the assets array changes between when the lightbox was opened and when next/prev is called (e.g., a search result updates, processing completes, or tab switch), the index may point to the wrong asset or be out of bounds. This is a race condition — clicking next/prev fast during a live search could show wrong images.
