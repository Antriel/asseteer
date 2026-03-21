---
# asseteer-60j7
title: Fix timeout including ZipGate wait time
status: todo
type: bug
priority: high
created_at: 2026-03-21T09:36:39Z
updated_at: 2026-03-21T09:36:39Z
parent: asseteer-k1go
---

## Problem

The 120s processing timeout for nested ZIP assets wraps the ENTIRE operation including ZipGate queue wait + cache load + file extraction + audio probing. In real-world testing with large bundles, ZipGate waits regularly exceed 100s (up to 134s observed), leaving no budget for actual processing. Small files (4MB mp3s that would process in <1s) fail with "Processing timed out after 120s" purely because they waited in the gate queue.

### Evidence from logs

```
[ZipGate] WARN slow ACTIVATE key='...sailingshipinastorm.zip::...' waited_ms=134191
[Worker] Failed to process asset 1 - Phat Phrog Studios - Sailing Ship In A Storm Ambience - 3 Minute.mp3: Some("Processing timed out after 120s")
```

The gate wait (134s) alone exceeds the timeout (120s).

## Root cause

In `processor.rs:176-245`, the `tokio::time::timeout` wraps `spawn_blocking` which contains BOTH:
1. `zip_cache::load_asset_bytes_cached()` — includes ZipGate acquire + cache load (can be 100+ seconds)
2. Symphonia audio probing (the actual processing, usually <1s)

```rust
let blocking_task = tokio::task::spawn_blocking(move || {
    let bytes = zip_cache::load_asset_bytes_cached(&asset_clone)?;  // ← gate wait here
    // ... probe audio ...
});
let timeout = if uses_nested_zip { NESTED_ZIP_PROCESSING_TIMEOUT } else { PROCESSING_TIMEOUT };
match tokio::time::timeout(timeout, blocking_task).await { ... }
```

## Suggested fix

Separate the timeout so ZipGate waiting is NOT counted toward processing timeout. Options:
1. **Move byte loading outside the timeout**: Load bytes first (with its own timeout or none), then wrap only the probing in the processing timeout
2. **Start timeout after bytes are loaded**: Signal from within the blocking task when bytes are ready, reset/start a timer from that point
3. **Give gate waiting its own timeout**: Separate, longer timeout (or no timeout) for cache acquisition vs a shorter one for actual processing

Option 1 is simplest. The byte loading already has its own slow-load warnings, and the ZipGate itself could have its own max-wait timeout if desired.

## Files to modify

- `src-tauri/src/task_system/processor.rs` — `process_audio()` (lines 160-298), timeout constants at top of file
- Potentially `process_image()` if it has the same pattern (check)

## Testing notes

- Existing tests in `src-tauri/src/task_system/` should still pass
- Hard to unit test gate contention, but the timeout logic change is straightforward
