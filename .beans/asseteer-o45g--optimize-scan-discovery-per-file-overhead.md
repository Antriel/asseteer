---
# asseteer-o45g
title: Optimize scan discovery per-file overhead
status: completed
type: task
priority: normal
created_at: 2026-03-23T07:42:34Z
updated_at: 2026-03-23T07:52:11Z
parent: asseteer-k1go
---

Low-hanging performance wins for the scan discovery phase that apply to all files (not just ZIPs). Currently ~10s for 70k plain files.

## Problem

The scan discovery loop in `discover_files_streaming()` (`commands/scan.rs`) has unnecessary per-file overhead that adds up across 70k+ files:

1. **Redundant stat syscall** — Line ~318 calls `std::fs::metadata(path)` to get file size and mtime, but WalkDir's `DirEntry` already has this information. On Windows, `entry.metadata()` returns cached metadata from `FindNextFile` (no syscall). On Linux/Mac, it's a stat either way — but the current code always does an *extra* stat on top of whatever WalkDir does.

2. **Excessive allocations in `compute_searchable_path()`** — Line ~466: for each path segment, `cumulative.clone()` allocates a growing string to look up in the HashSet. The HashSet key type is `(Option<String>, String)`, so every lookup requires owned strings. For 70k files with ~5 path segments each, that's ~350k unnecessary string allocations.

## Fixes

### 1. Use `entry.metadata()` instead of `std::fs::metadata(path)`

```rust
// Before (line ~318):
let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;

// After:
let metadata = entry.metadata().map_err(|e| e.to_string())?;
```

Same fix applies to the ZIP path (line ~356) where `std::fs::metadata(path)` is called for ZIP mtime.

**Impact**: Eliminates one stat syscall per file on all platforms. On Windows, this is essentially free (cached). On Linux/Mac, it goes from 2 stats to 1 per file.

### 2. Avoid cloning in `compute_searchable_path()`

The HashSet lookup currently requires `(Option<String>, String)` — owned types that force allocation on every probe. Options:

- Change the HashSet to store `(Option<Arc<str>>, Arc<str>)` or use a `HashSet<String>` with a combined key format (e.g., `"zip_file\0cumulative"`) that can be probed with a stack-allocated formatted string
- Or build the cumulative path in a reusable `String` buffer and use `.contains()` with a borrowed view (requires the `raw_entry` API or a newtype wrapper for `Borrow` impl)
- Simplest: use a `HashSet<String>` keyed on just the cumulative path string (since the zip_file dimension is None for filesystem paths and constant within a single ZIP), probed via `str` borrow

**Impact**: Eliminates ~350k string allocations for a 70k file scan.

## Testing

- Verify scan produces identical results (same assets discovered, same searchable_path values)
- Compare scan time before/after on a large non-ZIP folder

## Summary of Changes

Both fixes applied in `src-tauri/src/commands/scan.rs`:

1. **entry.metadata()** — Replaced `std::fs::metadata(path)` with `entry.metadata()` at both call sites (regular files and ZIP files). Eliminates one redundant stat syscall per file.

2. **Zero-alloc HashSet probing** — Changed the excludes set from `HashSet<(Option<String>, String)>` to `HashSet<String>` using a combined `"{zip}\0{path}"` key format. `compute_searchable_path()` now builds probe keys in a reusable `String` buffer and calls `contains(probe.as_str())` — zero allocations per lookup instead of ~5 clones per file. The `result` vec also now holds `&str` borrows instead of owned `String`s.

All 62 tests pass (1 pre-existing flaky test in zip_cache).
