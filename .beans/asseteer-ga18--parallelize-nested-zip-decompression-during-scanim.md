---
# asseteer-ga18
title: Parallelize nested ZIP decompression during scan/import
status: todo
type: feature
priority: normal
created_at: 2026-03-21T12:40:32Z
updated_at: 2026-03-21T12:40:43Z
parent: asseteer-k1go
---

Parallelize the scan discovery phase so multiple outer ZIPs nested ZIP archives are decompressed concurrently, bounded by the shared memory budget from zip_cache.


## Problem

The scan/import discovery phase (`commands/scan.rs`) decompresses nested ZIP archives fully into memory to enumerate their entries. This runs on a **single blocking thread** — each outer ZIP is opened and all its nested ZIPs decompressed sequentially. With nested ZIPs averaging ~633 MB decompressed (up to 1.2 GB), a folder with 10+ outer ZIPs containing nested archives spends most of its time in sequential decompression.

The processing phase already parallelizes nested ZIP access via the multi-slot ZipCache (asseteer-5nvp), but the scan phase that precedes it is entirely serial.

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
- `zip_cache::budget_bytes()` — memory budget already computed from system RAM

## Design: Parallel outer ZIP processing with rayon

### Why outer ZIP level (not nested ZIP level)

Parallelizing at the outer ZIP level is simpler and sufficient:
- Most scan time is spent decompressing nested ZIPs inside outer ZIPs
- Each outer ZIP is independent (different files, no shared state)
- Parallelizing nested ZIP decompression within a single outer ZIP is harder (ZipArchive borrows prevent concurrent entry reads) and provides less benefit

### Approach

1. **Phase 1: Filesystem walk (serial)** — WalkDir collects all paths into two lists: `zip_paths: Vec<PathBuf>` and regular files go straight to chunk buffer. This is fast (metadata only, no heavy I/O).

2. **Phase 2: Flush regular files** — Send remaining regular file chunk through the channel.

3. **Phase 3: ZIP files (parallel via rayon)** — Process outer ZIPs in parallel using `rayon::par_iter()`, bounded by memory:

```
rayon thread pool (default = CPU count)
  Thread 1: discover_zip_streaming(zip_a.zip) -> tx.blocking_send(chunk)
  Thread 2: discover_zip_streaming(zip_b.zip) -> tx.blocking_send(chunk)
  Thread 3: discover_zip_streaming(zip_c.zip) -> tx.blocking_send(chunk)
  (bounded by memory semaphore)
```

### Memory bounding

Use a counting semaphore to limit concurrent ZIP processing based on `zip_cache::budget_bytes()`:

```rust
let max_concurrent = std::cmp::max(2, zip_cache::budget_bytes() / (1024 * 1024 * 1024));
```

Each rayon thread acquires a permit before starting a ZIP, releases it when done. This prevents OOM when processing folders with many large outer ZIPs.

Implementation note: `std::sync::Semaphore` was removed from std. Use crossbeam (already a dependency) or a simple `Mutex<usize>` + `Condvar` pair, or `tokio::sync::Semaphore` with `block_in_place`.

### Channel considerations

The current `mpsc::channel` (`tokio::sync::mpsc`) works from rayon threads via `tx.blocking_send()`. Each rayon thread needs its own `tx` clone. The channel buffer (32 chunks) provides back-pressure if insertion falls behind.

### Progress tracking

Atomic counters already use `Relaxed` ordering and are safe for concurrent access from multiple rayon threads. `current_path` in progress events becomes less meaningful with parallel processing (multiple ZIPs in flight); could show "processing N ZIPs in parallel" instead.

## Implementation sketch

```rust
fn discover_files_streaming(
    root_path: &str,
    tx: mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: Arc<ScanProgress>,
) -> Result<(), String> {
    let mut zip_paths: Vec<PathBuf> = Vec::new();
    let mut chunk: Vec<DiscoveredAsset> = Vec::with_capacity(CHUNK_SIZE);

    // Phase 1: Walk filesystem, collect ZIP paths, send regular files
    for entry in WalkDir::new(root_path).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() { continue; }

        match path.extension().and_then(|e| e.to_str()) {
            Some("zip") => zip_paths.push(path.to_path_buf()),
            Some(ext) if is_supported(ext) => {
                // Add to chunk, send when full (same as current)
            }
            _ => {}
        }
    }
    // Flush remaining regular files
    if !chunk.is_empty() { tx.blocking_send(chunk).ok(); }

    // Phase 2: Process ZIPs in parallel
    let max_concurrent = std::cmp::max(2, zip_cache::budget_bytes() / (1024 * 1024 * 1024));
    // counting semaphore via Mutex+Condvar or crossbeam

    zip_paths.par_iter().for_each(|zip_path| {
        // Acquire memory permit
        // discover_zip_streaming(zip_path, tx.clone(), progress.clone(), ...)
        // Release memory permit
    });

    Ok(())
}
```

## Files to modify

- `src-tauri/src/commands/scan.rs` — Restructure `discover_files_streaming()` into walk + parallel ZIP processing phases
- Possibly extract `discover_zip_streaming()` / `discover_zip_recursive_streaming()` to be `Send + Sync` compatible (they already should be since they only use local state + channel sender)

## Testing considerations

- Test with a folder containing multiple outer ZIPs with nested ZIPs
- Verify memory usage stays bounded (watch for concurrent decompression OOM)
- Verify `files_found` / `zips_scanned` counters are still accurate
- Verify insertion order doesn't matter (it shouldn't — assets are identified by path, not insertion order)
- Edge cases: folder with 100+ small ZIPs, folder with 1 huge ZIP, empty ZIPs

## Open questions

- [ ] Should we keep the progress `current_path` field, or change to "processing N ZIPs in parallel"?
- [ ] Is the walk phase fast enough to remain serial, or should we also consider parallel directory traversal?
- [ ] Should we sort ZIPs by file size descending (process largest first) like the processing dispatcher does?
- [ ] What's the right default for max_concurrent? budget / 1GB or something more conservative?
