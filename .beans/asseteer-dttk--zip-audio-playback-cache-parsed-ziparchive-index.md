---
# asseteer-dttk
title: 'ZIP audio playback: cache parsed ZipArchive index'
status: scrapped
type: task
priority: normal
created_at: 2026-03-20T07:44:28Z
updated_at: 2026-03-20T07:53:21Z
parent: asseteer-kvnt
---

Even with the inner-ZIP bytes cached in memory, ZipArchive::new() re-parses the central directory of the 1GB archive on every entry read (~400ms). Cache the parsed archive structure (file index) to avoid this per-call overhead.

## Reasons for Scrapping

Added parse_ms/extract_ms logging to measure ZipArchive::new() separately from entry extraction. parse_ms was consistently 0ms across all tested files — the central directory parse is negligible. The 300-400ms on cache hits scales with file size (extract_ms), confirming it's pure decompression time, not index parsing. Not worth the complexity.
