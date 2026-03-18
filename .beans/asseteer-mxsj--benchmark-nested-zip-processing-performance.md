---
# asseteer-mxsj
title: Benchmark nested ZIP processing performance
status: todo
type: task
created_at: 2026-03-18T11:17:26Z
updated_at: 2026-03-18T11:17:26Z
parent: asseteer-526f
---

Nested ZIPs (ZIP-inside-ZIP) can't seek into the inner archive without decompressing the outer entry first. Each file access likely decompresses the entire inner ZIP into memory. Benchmark to quantify actual cost, then design a caching strategy (hold decompressed inner ZIP in memory/tempfile for consecutive accesses).
