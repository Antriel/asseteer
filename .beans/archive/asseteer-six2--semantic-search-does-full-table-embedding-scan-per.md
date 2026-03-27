---
# asseteer-six2
title: Semantic search does full-table embedding scan per query
status: scrapped
type: bug
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-03-16T11:54:50Z
parent: asseteer-bh0n
---

search_audio_semantic fetches all candidate embeddings into memory (etch_all, line ~102 in src-tauri/src/commands/search.rs), computes cosine similarity for every row, then sorts in memory and truncates (lines ~131-132). This is O(N) per query and will degrade sharply as audio corpus grows. Introduce candidate prefiltering/indexing or ANN search to keep latency stable.


## Reasons for Scrapping

Brute-force cosine similarity over all embeddings is fast enough even at 100k+ assets — it's just vector dot products. ANN indexing (HNSW, IVF) would add significant complexity for marginal gain at this scale. There's no meaningful prefiltering dimension that wouldn't hurt recall. This is premature optimization for a problem that doesn't exist at the current (or foreseeable) scale. Can revisit if the corpus grows to millions of assets.
