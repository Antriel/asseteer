---
# asseteer-six2
title: Semantic search does full-table embedding scan per query
status: todo
type: bug
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-02-14T07:32:05Z
parent: asseteer-bh0n
---

search_audio_semantic fetches all candidate embeddings into memory (etch_all, line ~102 in src-tauri/src/commands/search.rs), computes cosine similarity for every row, then sorts in memory and truncates (lines ~131-132). This is O(N) per query and will degrade sharply as audio corpus grows. Introduce candidate prefiltering/indexing or ANN search to keep latency stable.
