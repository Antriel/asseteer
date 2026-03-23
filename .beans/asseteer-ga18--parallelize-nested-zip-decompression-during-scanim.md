---
# asseteer-ga18
title: Parallelize nested ZIP decompression during scan/import
status: todo
type: feature
priority: normal
created_at: 2026-03-21T12:40:32Z
updated_at: 2026-03-23T00:00:00Z
parent: asseteer-k1go
---

Parallelize the scan discovery phase using `rayon::scope` so ZIP decompression (especially nested ZIPs) runs concurrently, bounded by the shared ZipCache memory budget.


## Problem

The scan/import discovery phase (`commands/scan.rs`) decompresses nested ZIP archives fully into memory to enumerate their entries. This runs on a **single blocking thread** — each outer ZIP is opened and all its nested ZIPs decompressed sequentially. With nested ZIPs typically 200 MB - 1 GB decompressed, and outer ZIPs containing ~12 of them, a folder with several such bundles spends most of its time in sequential decompression.

The processing phase already parallelizes nested ZIP access via the multi-slot ZipCache, but the scan phase that precedes it is entirely serial. The scan does not use ZipCache at all — it eagerly decompresses into throwaway buffers.

## Current architecture

### Discovery flow (`scan.rs`)

```
add_folder()
  spawn_blocking(discover_files_streaming)     <- SINGLE THREAD
    WalkDir::new(root_path)                     <- Sequential filesystem walk
      Regular files -> chunk buffer
      .zip files -> discover_zip_streaming()
        discover_zip_recursive_streaming()
          Collect entries_info (two-pass to avoid borrow conflicts)
          .zip entries -> entry.read_to_end(&mut buffer)  <- BOTTLENECK
                          ZipArchive::new(Cursor::new(buffer))
                          recursive call
          Media entries -> DiscoveredAsset -> chunk buffer
```

### Key functions

- `discover_files_streaming()` (line ~293): WalkDir loop, dispatches to `discover_zip_streaming()` for each `.zip` file
- `discover_zip_streaming()` (line ~430): Opens outer ZIP, calls `discover_zip_recursive_streaming()`
- `discover_zip_recursive_streaming()` (line ~519): Two-pass entry enumeration. For nested `.zip` entries: `entry.read_to_end(&mut buffer)` then `ZipArchive::new(Cursor)` then recursive call. For media: creates `DiscoveredAsset` and adds to chunk.

### Streaming channel

```rust
let (tx, mut rx) = mpsc::channel::<Vec<DiscoveredAsset>>(32);  // line 114
const CHUNK_SIZE: usize = 200;                                   // line 33
```

Discovery (blocking thread) sends chunks of 200 assets. Insertion (async task) consumes and inserts in transactions concurrently.

### Progress tracking

Atomic counters (Relaxed ordering): `files_found`, `files_inserted`, `zips_scanned`, `discovery_complete`. Progress events emitted every 100ms from both discovery and insertion sides.

### Error handling

Soft errors: `eprintln!("Warning: ...")` + continue (skip that ZIP/entry). Fatal errors: return `Err(String)` to stop the scan.

### Existing dependencies

- `rayon = "1.10"` — already in Cargo.toml, not currently used in scan path
- `zip` crate with `deflate-zlib-ng` (SIMD-accelerated) — already optimizes decompression speed
- `zip_cache` — memory-budgeted cache with multi-reader support, LRU eviction, reference counting via `ActiveEntryGuard`

## Design: Pipelined parallel discovery with `rayon::scope`

### Key design decisions

1. **Parallelize at the nested ZIP level, not just outer ZIP level.** The bottleneck is decompressing nested ZIPs. A single outer ZIP with 12 nested ZIPs benefits from parallel nested reads (each thread opens its own file handle to the outer ZIP via `ZipArchive::by_index()`).

2. **Use the shared ZipCache during scan.** Same bytes, same budget — no reason for a separate memory management scheme. Nested ZIP bytes stored in ZipCache during scan may still be warm when processing starts (especially for smaller imports). Even when they aren't, the budget reasoning is identical.

3. **Pipeline, not batch.** The walk runs concurrently with ZIP processing — ZIPs start decompressing as soon as they're discovered, rather than waiting for the walk to finish. `rayon::scope` blocks until all spawned work completes, giving a natural "discovery complete" signal.

4. **STORE vs DEFLATE is irrelevant to the design.** `entry.read_to_end()` abstracts the compression method. The parallelization strategy is identical regardless.

### Architecture

```
rayon::scope(|s| {
    // Producer: filesystem walk (one task)
    s.spawn(|_| {
        WalkDir -> for each entry:
          regular file -> send through asset channel
          .zip file -> s.spawn(process_outer_zip)
    });

    // process_outer_zip runs in rayon pool:
    //   Open ZipArchive, enumerate entries
    //   media entries -> send through asset channel
    //   nested .zip entries -> for each: s.spawn(process_nested_zip)

    // process_nested_zip:
    //   Open NEW ZipArchive handle to same outer file
    //   Read nested ZIP bytes via by_index() -> store in ZipCache
    //   Enumerate nested ZIP contents from cached bytes
    //   media entries -> send through asset channel
    //   deeper nesting -> spawn more tasks
});
// scope returns only when ALL tasks done -> signal discovery_complete
```

### Why this is simpler than the previous 3-phase plan

- **No phases** — walk and ZIP processing overlap naturally
- **No semaphore to build** — ZipCache already does memory bounding with LRU eviction
- **No collect-then-iterate** — ZIPs start processing as soon as found
- **Recursive nesting maps to recursive spawn** — the code structure stays close to current
- **Completion tracking is free** — `rayon::scope` blocks until done

### Memory bounding via ZipCache

ZipCache already handles this:
- Loading coordination: first thread decompresses, others wait on condvar
- Reference counting via `ActiveEntryGuard` prevents eviction while in use
- LRU eviction when over budget (only evicts unpinned entries)
- Temporary over-budget is allowed if all entries are pinned (existing behavior)

For scan, each nested ZIP decompression goes through ZipCache. If the budget is full, older unpinned entries get evicted. This is the same mechanism processing uses.

### Channel considerations

The current `tokio::sync::mpsc` channel works from rayon threads via `tx.blocking_send()`. Each spawned task clones the sender. The channel buffer (32 chunks) provides back-pressure. The chunking (200 assets per chunk) should use thread-local buffers to avoid contention — each task accumulates its own chunk and sends when full.

### Progress tracking

Atomic counters (Relaxed ordering) are already safe for concurrent access. `current_path` becomes less meaningful with parallel processing — could show count of ZIPs in flight or just drop it.

### Opening multiple handles to the same outer ZIP

Each nested ZIP task opens its own `ZipArchive::new(File::open(outer_path))`. This is safe — each instance has its own file descriptor and seek position. On SSDs this enables true parallel I/O. The `zip` crate's `by_index()` seeks to the entry's offset and reads/decompresses it independently.

## Implementation approach

### Changes to `discover_files_streaming()`

Replace the single-threaded WalkDir+ZIP loop with a `rayon::scope` block. The walk becomes a producer task that spawns ZIP tasks as it discovers them.

### Changes to `discover_zip_streaming()` / `discover_zip_recursive_streaming()`

- Accept a rayon `Scope` parameter to spawn child tasks for nested ZIPs
- For nested ZIPs: instead of reading bytes into a local buffer, store them in ZipCache and enumerate from cached bytes
- Each nested ZIP task opens its own file handle to the outer ZIP

### Thread-local chunk buffers

Each rayon task maintains its own `Vec<DiscoveredAsset>` chunk buffer. When full (or task completes), sends through the channel. This avoids contention on a shared buffer.

## Files to modify

- `src-tauri/src/commands/scan.rs` — Restructure discovery to use `rayon::scope`, integrate with ZipCache
- `src-tauri/src/zip_cache.rs` — May need a lighter-weight "store and enumerate" path for scan (current `load_asset_bytes_cached` returns individual asset bytes, but scan needs to enumerate entries from cached nested ZIP bytes)

## Testing considerations

- Test with a folder containing multiple outer ZIPs with nested ZIPs
- Verify memory usage stays bounded (ZipCache eviction works correctly under scan load)
- Verify `files_found` / `zips_scanned` counters are still accurate
- Verify insertion order doesn't matter (assets are identified by path, not insertion order)
- Edge cases: folder with 100+ small ZIPs, folder with 1 huge ZIP, empty ZIPs, deeply nested ZIPs
- Verify cached nested ZIP bytes are usable by processing phase (no double-decompression for small imports)
