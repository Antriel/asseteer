---
# asseteer-2n9u
title: Lazy-load thumbnails on demand
status: todo
type: feature
created_at: 2026-03-16T11:42:37Z
updated_at: 2026-03-16T11:42:37Z
---

Instead of processing all thumbnails upfront, generate them lazily when assets become visible. Avoids forcing full processing before thumbnails appear. Also fixes the issue where frontend doesn't know if a thumbnail was skipped (small image) or just not yet generated.
