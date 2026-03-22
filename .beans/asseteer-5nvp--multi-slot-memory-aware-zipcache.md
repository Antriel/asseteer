---
# asseteer-5nvp
title: Multi-slot memory-aware ZipCache
status: completed
type: feature
priority: normal
created_at: 2026-03-21T09:37:45Z
updated_at: 2026-03-21T12:33:36Z
parent: asseteer-k1go
blocked_by:
    - asseteer-fbpx
---

## Problem

The current ZipCache (`zip_cache.rs`) is a single-slot cache: only one decompressed nested ZIP can be held in memory at a time. When multiple nested ZIPs need processing, every key switch forces a full eviction and re-decompression. With the batch grouping improvements (asseteer-fbpx), thrashing will be greatly reduced, but there are still scenarios where multiple concurrent accesses would benefit from caching multiple ZIPs:

- CLAP processing (concurrency=1) and Audio processing (concurrency=100) could run simultaneously on different categories, both needing different nested ZIPs
- Future multi-category parallel processing
- Very large bundles with many small nested ZIPs where round-robin between a few would benefit from keeping 2-3 hot

## Bug: Cross-category cache interference

When ANY category completes or is stopped, `zip_cache::clear()` is called globally (`work_queue.rs:477` and `work_queue.rs:530`). This means if Audio processing finishes while CLAP is still running and using the cache, the cache gets cleared. Active `Arc<Vec<u8>>` clones survive (no crash), but the next CLAP cache access forces a full re-decompression.

This must be addressed as part of the multi-slot cache work:
- With per-entry reference counting and pinning, `clear()` should only evict unpinned entries
- Or: `clear()` should check if any other category is still running before evicting
- The staged dispatcher (asseteer-fbpx) should also be aware of this — when one category finishes, it shouldn't nuke another category's active cache entries

## Current architecture

```rust
// zip_cache.rs — single static cache slot
static CACHE_STATE: Mutex<CacheState> = ...;
enum CacheState {
    Empty,
    Loading(String),
    Ready(CachedInnerZip), // single Arc<Vec<u8>> + key
}

// ZipGate — single active key
struct ActiveKeyState {
    active_key: Option<String>,  // only ONE key active at a time
    active_users: usize,
}
```

All threads serialize through one active key. The cache holds one decompressed ZIP. Decompressed sizes observed in logs: 682 MB to 1270 MB per nested ZIP.

## Design considerations

### Memory budget
- Must be friendly to other processes — don't consume all available RAM
- Nested ZIPs can be very large (1+ GB decompressed)
- Should query available system memory and set a budget (e.g., 50% of free memory, capped at some max like 4GB)
- Budget should be configurable or at least adaptive

### Eviction strategy
- LRU makes sense: evict the least-recently-used cache entry when budget exceeded
- Before loading a new ZIP, check if it fits within remaining budget
- If it doesn't fit, evict LRU entries until enough space is freed
- If a single ZIP exceeds the entire budget, still allow it (degenerate to single-slot behavior)

### Concurrency model changes
- ZipGate currently serializes by key (only one key active). With multi-slot, multiple keys can be active simultaneously
- Each cached entry needs its own reference count (`active_users`)
- Gate should only block when a new key needs loading AND the cache is full AND all entries are actively in use (can't evict any)
- Otherwise, concurrent access to different cached ZIPs should proceed in parallel

### API compatibility
- `load_asset_bytes_cached(asset)` should remain the same external API
- `acquire_active_nested_zip_key(key)` and `get_cached_nested_zip_bytes()` internals would change
- `ActiveKeyGuard` RAII pattern should be preserved but per-entry rather than global

### System memory detection
- On Windows: use `sysinfo` crate or Win32 `GlobalMemoryStatusEx`
- Cross-platform: `sysinfo` crate (`System::available_memory()`)
- Check periodically or before each cache fill, not just at startup (memory pressure changes)
- Consider: should the cache shrink proactively if system memory gets low? (OS-friendly behavior)

## Rough implementation sketch

```rust
struct CacheEntry {
    key: String,
    data: Arc<Vec<u8>>,
    size_bytes: usize,
    last_accessed: Instant,
    active_users: AtomicUsize, // can't evict if > 0
}

struct MultiSlotCache {
    entries: Vec<CacheEntry>,      // or HashMap<String, CacheEntry>
    total_cached_bytes: usize,
    memory_budget: usize,          // dynamically calculated
}
```

## Dependencies

- Should be done AFTER asseteer-fbpx (batch grouping) since grouping reduces thrashing significantly and may change how much benefit multi-slot provides
- Needs `sysinfo` crate (or similar) added to Cargo.toml for memory detection

## Open questions

- [ ] What percentage of free memory is safe to use? 50%? Configurable in settings?
- [ ] Should we cap at a hard maximum regardless (e.g., 4GB) to prevent runaway usage?
- [ ] Should we expose cache stats to the UI (current usage, hit rate)?
- [ ] Is `sysinfo` the right crate, or is there something lighter-weight for just memory info?
- [ ] Should the cache proactively shrink under memory pressure, or only manage its own budget?
- [ ] With multi-slot cache + improved batching, is the ZipGate concept still needed, or does it become per-entry locking?

## Files to modify

- `src-tauri/src/zip_cache.rs` — major rewrite of cache internals
- `src-tauri/Cargo.toml` — add `sysinfo` or equivalent dependency
- Possibly `src-tauri/src/task_system/work_queue.rs` — if ZipGate behavior changes affect worker coordination


## Future use case: parallel scan/import (not in scope, but informs design)

The memory budget system built for this cache should be designed as a reusable utility that other subsystems can leverage. The scan/import phase is one such consumer: nested ZIPs must be fully decompressed into memory to enumerate their entries (~633 MB average, up to 1.2 GB), currently fully serial. With a shared memory budget, the scan phase could parallelize outer ZIP processing while respecting the same memory limits. This is a separate piece of work, but the memory budget API should be extracted as a standalone module rather than coupled to the ZipCache internals.

Key scan code reference: `src-tauri/src/commands/scan.rs` — `discover_zip_recursive_streaming()` decompresses nested ZIPs via `entry.read_to_end(&mut buffer)` (line 568-571) on a single blocking thread. `rayon` is already a dependency.


## Revised design: Cache-aware dispatcher (supersedes earlier concurrency model section)

### Unified dispatcher + cache design

With multi-slot cache, the dispatcher from asseteer-fbpx gets upgraded to be cache-aware. Instead of dispatching one key group at a time, it can dispatch multiple simultaneously based on available memory:

```
Dispatcher loop:
  1. Next key group is [B1,B2,B3] (key B, ~800 MB decompressed)
  2. Ask cache: "can I load key B?" → checks memory budget
  3. YES (enough free memory) → dispatch B batches, pin key B in cache
     - Multiple key groups can be active simultaneously
     - Workers for key A and key B run in parallel
  4. NO (cache full, all entries pinned) → dispatch non-ZIP batches to fill workers
  5. Key A completes → unpin A, cache can evict → re-check, dispatch next key group
```

### ZipGate replacement

The current ZipGate (global single-active-key serialization) is fully replaced by:
- **Cache pinning**: each dispatched key group pins its cache entry (can't be evicted)
- **Per-entry reference counting**: `active_users` per cache entry, not global
- **Dispatcher gating**: only the dispatcher decides when to send new key groups, not the workers

Workers never race for the gate — they just call `load_asset_bytes_cached()` which hits a pinned cache entry directly. No condvar waits, no convoy effect.

### Memory budget governs parallelism

The number of simultaneously active ZIP keys is naturally bounded by memory:
- 4 GB budget with 1 GB ZIPs → max 4 keys active at once
- 4 GB budget with 200 MB ZIPs → max 20 keys active at once
- Single ZIP exceeding entire budget → degenerates to single-slot (still works)

This self-tunes to the workload and available system resources.

### Dispatcher interface (designed for upgrade from asseteer-fbpx)

```rust
trait CacheBudget {
    /// Can a new key of this size be loaded? (checks available memory)
    fn can_load(&self, estimated_size: usize) -> bool;
    /// Pin key in cache (increment ref count, prevent eviction)
    fn pin(&self, key: &str) -> PinGuard;
    /// Estimate decompressed size before loading (from ZIP central directory)
    fn estimate_size(key: &str) -> usize;
}
```

The asseteer-fbpx staged dispatch should use a simple version of this interface (single-slot: `can_load` returns true only when no key is active), making the upgrade to multi-slot a matter of swapping the implementation.


## Implementation Checklist

- [x] Phase 1: Cache data structure rewrite (zip_cache.rs)
- [x] Phase 2: Fix cross-category interference (work_queue.rs clear → evict_unpinned)
- [x] Phase 3: Multi-group dispatcher (work_queue.rs concurrent dispatch)
- [x] Phase 4: Cargo.toml (Win32_System_SystemInformation feature)
- [x] Phase 5: Logging
- [x] Tests pass (62/62)


## Summary of Changes

### zip_cache.rs — Major rewrite
- **Multi-slot HashMap cache**: Replaced single-slot `CacheState` enum with `HashMap<String, CacheEntry>` supporting multiple simultaneous entries
- **Per-entry state machine**: `EntryState::Loading` / `EntryState::Ready` with per-entry `active_users` ref counting and LRU `last_access` counter
- **Memory budget**: Queries `GlobalMemoryStatusEx` on Windows for available RAM, sets budget to 50% of available (clamped to 1–8 GB range)
- **LRU eviction**: `evict_for_budget()` removes least-recently-used unpinned entries when budget exceeded
- **Unified lock model**: Replaced 4 globals (CACHE + CACHE_READY + ACTIVE_KEY + ACTIVE_KEY_READY) with 2 (CACHE + CACHE_CHANGED) via `once_cell::sync::Lazy`
- **ActiveEntryGuard**: Per-entry RAII guard replacing global ActiveKeyGuard
- **New public API**: `evict_unpinned()`, `budget_bytes()`, `cached_bytes()`, `entry_count()`
- **4 new tests**: multi-entry, LRU eviction, pinned entry protection, evict_unpinned

### work_queue.rs — Dispatcher upgrade + cross-category fix
- **Concurrent multi-group dispatch**: Replaced sequential `for group in zip_groups` with `JoinSet` + `Semaphore` bounded by memory budget (~1 concurrent group per GB)
- **Cross-category fix**: Replaced `zip_cache::clear()` with `zip_cache::evict_unpinned()` at category completion and stop — pinned entries used by other running categories are preserved

### Cargo.toml
- Added `Win32_System_SystemInformation` feature to existing `windows-sys` dependency for `GlobalMemoryStatusEx`
