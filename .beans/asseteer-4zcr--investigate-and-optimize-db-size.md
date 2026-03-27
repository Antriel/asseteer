---
# asseteer-4zcr
title: Investigate and optimize DB size
status: completed
type: task
priority: normal
created_at: 2026-03-16T11:42:44Z
updated_at: 2026-03-27T08:02:22Z
---

Measure actual database size with realistic asset counts. Investigate what's taking up space and whether there are opportunities to reduce it.


## Investigation Summary (2026-03-27, 1.5M assets, 3.95 GB DB)

| Component | Size |
|-----------|------|
| `audio_embeddings` | 1,290 MB (32.7%) |
| `assets_fts_sub` (trigram) | 1,142 MB (28.9%) |
| `assets` table + indexes | 1,033 MB (26.2%) |
| `assets_fts_word` (unicode61) | 412 MB (10.4%) |
| `directories` | 24 MB |
| `audio_metadata` + `image_metadata` | 9 MB |

### Why each component is the size it is

**Embeddings (1.29 GB):** 326K audio files × 512 f32 values = 638 MB raw data, ~2× B-tree page overhead. Inherent to storing 512-dim CLAP vectors. Could halve raw size with f16 quantization but that risks search quality degradation — not worth it.

**Trigram FTS (1.14 GB):** The `assets_fts_sub` trigram tokenizer generates ~N-2 tokens per filename (one per 3-char window). With 1.5M filenames averaging 23 chars, that's ~30M trigram entries. This is the cost of substring search and is expected.

**Assets table + indexes (1 GB):** 1.5M rows with `searchable_path` (full path stored as text) is the bulk. The new compound indexes (`idx_assets_type_filename`, `idx_assets_folder_type_filename`) add ~113 MB but deliver 6,300× browse speedup — well worth it.

### Conclusion

No actionable optimizations. All storage is proportional to the data being stored and features being supported. The DB is operating efficiently (99% of pages store data, <1% freelist).
