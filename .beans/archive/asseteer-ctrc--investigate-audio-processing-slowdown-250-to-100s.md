---
# asseteer-ctrc
title: 'Investigate audio processing slowdown: 250 to 100/s during non-nested ZIP processing'
status: completed
type: task
priority: normal
created_at: 2026-03-25T07:16:00Z
updated_at: 2026-03-25T09:37:03Z
---

Processing 103k assets from 52 ZIPs starts at 250/s but degrades to 100/s. SSD usage drops, CPU stays high but with low heat (indicating wait, not computation).


## Context
- 103k assets in 52 ZIPs (~2000 assets/ZIP avg), batched into 6488 batches of 16
- Write rate is fine (few KB/s after rusqlite switch with wal_autocheckpoint=0)
- High CPU (55%) but low heat = I/O wait or runtime overhead, not computation

## Investigation Plan
- [ ] Add per-phase timing to worker loop: bulk ZIP extraction time vs Symphonia probe time vs RwLock wait vs spawn_blocking overhead
- [x] Log batch extraction stats (entries extracted, total bytes, elapsed ms)
- [ ] Check if ZIP entry processing order correlates with position in archive (sequential vs random seek)
- [ ] Check if WAL growth during processing causes read contention (frontend queries scanning growing WAL)
- [ ] Profile spawn_blocking pool utilization
- [ ] Consider reducing worker count for I/O-bound non-nested ZIP workloads

## Hypotheses (ranked by likelihood)
1. ZIP seek patterns: later batches access entries scattered throughout large ZIPs, causing random I/O
2. Symphonia probe cost varies by encoding — VBR/complex MP3s cluster later
3. spawn_blocking context switching overhead (46 concurrent tasks for trivial work)
4. WAL growth causing frontend read contention that competes for SSD
5. tokio RwLock contention on current_file (23 workers, 2 writes/asset)

## Implementation Notes\n\nAdded backend audio-processing instrumentation that writes JSONL run logs to processing-logs/ next to the app database. Events include per-asset probe timing, per-ZIP-batch extraction summary, periodic window summaries, and run completion/stop totals.\n


## Summary of Changes

Investigation complete. Root cause identified: non-nested ZIP extraction slowdown is caused by repeated central directory parsing of large ZIP archives (40K+ entries) by concurrent workers.

### Key findings:
- `probe_ms` avg 0.02ms — Symphonia probing is not the issue
- `extract_ms` 3.6-16s per batch of 16 entries from 40K-entry ZIPs
- The 3.6s baseline for small batches is almost entirely `ZipArchive::new()` parsing the central directory
- ~23 workers concurrently parsing the same 40K-entry CD = the bottleneck
- High CPU + low heat = allocation/parsing overhead, not I/O or computation

### Instrumentation added (uncommitted, to be cleaned up):
- Per-asset audio probe timing with zip metadata (archive index, compressed/uncompressed sizes, compression method)
- Per-ZIP-batch extraction summary (extract_ms, total_bytes, effective MB/s)
- Periodic window summaries with aggregate stats
- JSONL run logs written to processing-logs/ directory
- Fixed expensive full-archive `by_index` metadata loop — replaced with per-entry `index_for_name` (central-directory only, no seeks)
- Added `compression` field to track Store vs Deflate per entry

### Follow-up: asseteer-50a1 (staged dispatch + shared archive optimization)
