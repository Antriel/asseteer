---
# asseteer-jbky
title: Duplicated search result construction in search.rs
status: completed
type: task
priority: normal
created_at: 2026-03-20T11:45:01Z
updated_at: 2026-03-21T08:35:55Z
parent: asseteer-c0lx
---

`search_audio_semantic` (lines 131-153) and `search_audio_by_similarity` (lines 200-222) have identical result-building code:

```rust
ranked.into_iter()
    .filter_map(|r| {
        metadata.get(&r.asset_id).map(|m| SemanticSearchResult {
            id: m.id,
            filename: m.filename.clone(),
            // ... 13 more fields identically mapped ...
            similarity: r.similarity,
        })
    })
    .collect()
```

**Fix**: Extract a `build_search_results(ranked, metadata)` helper or implement `From` on SemanticSearchResult.


## Summary of Changes

Extracted the duplicated `filter_map` result-building block from `search_audio_semantic` and `search_audio_by_similarity` into a `build_search_results(ranked, metadata)` helper function. Both call sites now use the helper.
