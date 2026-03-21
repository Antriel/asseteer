---
# asseteer-60j7
title: Separate cache/gate wait from processing timeouts for all nested-ZIP processors
status: todo
type: bug
priority: high
created_at: 2026-03-21T09:36:39Z
updated_at: 2026-03-21T09:36:39Z
parent: asseteer-k1go
---

## Problem

Processing timeouts for nested ZIP assets wrap the ENTIRE operation including ZipGate queue wait + cache load + file extraction + actual processing. In real-world testing with large bundles, ZipGate waits regularly exceed 100s (up to 134s observed), leaving no budget for actual processing. Small files (4MB mp3s that would process in <1s) fail with timeout errors purely because they waited in the gate queue.

### Evidence from logs

```
[ZipGate] WARN slow ACTIVATE key='...sailingshipinastorm.zip::...' waited_ms=134191
[Worker] Failed to process asset 1 - Phat Phrog Studios - Sailing Ship In A Storm Ambience - 3 Minute.mp3: Some("Processing timed out after 120s")
```

The gate wait (134s) alone exceeds the timeout (120s).

## Affected code paths

All three nested-ZIP processing paths have the same pattern — `tokio::time::timeout` wrapping `spawn_blocking` which contains BOTH `load_asset_bytes_cached()` (gate wait) and the actual work:

### 1. Audio processing (`processor.rs:176-245`)
- Timeout: 120s for nested ZIP, 30s otherwise
- `load_asset_bytes_cached()` + Symphonia audio probing inside one `spawn_blocking`

```rust
let blocking_task = tokio::task::spawn_blocking(move || {
    let bytes = zip_cache::load_asset_bytes_cached(&asset_clone)?;  // ← gate wait here
    // ... probe audio ...
});
let timeout = if uses_nested_zip { NESTED_ZIP_PROCESSING_TIMEOUT } else { PROCESSING_TIMEOUT };
match tokio::time::timeout(timeout, blocking_task).await { ... }
```

### 2. Image processing (`processor.rs:40-67`)
- Timeout: 30s (no nested ZIP distinction — even worse)
- `load_asset_bytes_cached()` + image decode + optional thumbnail generation inside one `spawn_blocking`

```rust
let result = tokio::time::timeout(
    PROCESSING_TIMEOUT,  // always 30s, no nested ZIP distinction
    tokio::task::spawn_blocking(move || {
        let bytes = zip_cache::load_asset_bytes_cached(&asset_clone)?;  // ← gate wait
        let img = image::load_from_memory(&bytes)?;
        // ... dimensions + optional thumbnail ...
    }),
).await;
```

### 3. Thumbnail generation on demand (`processor.rs:144-158`)
- Timeout: 30s
- Same pattern: `load_asset_bytes_cached()` + image decode + resize in one `spawn_blocking`

## Suggested fix

Separate the timeout so ZipGate/cache waiting is NOT counted toward processing timeout. Options:
1. **Move byte loading outside the timeout**: Load bytes first (with its own timeout or none), then wrap only the actual processing in the timeout
2. **Start timeout after bytes are loaded**: Signal from within the blocking task when bytes are ready, reset/start a timer from that point
3. **Give gate waiting its own timeout**: Separate, longer timeout (or no timeout) for cache acquisition vs a shorter one for actual processing

Option 1 is simplest. The byte loading already has its own slow-load warnings, and the ZipGate itself could have its own max-wait timeout if desired.

### Cleanup of timed-out blocking tasks

Note: when `tokio::time::timeout` fires, it drops the future but the `spawn_blocking` task continues running in the background (tokio doesn't abort blocking tasks). The current code has no cleanup/abort path for this. After the timeout fix this becomes less urgent (fewer false timeouts), but should be considered — a truly stuck blocking task would leak a thread. A pragmatic approach: the blocking task can check an `AtomicBool` cancellation flag periodically if needed, but in practice audio probing and image decoding are fast once bytes are loaded.

## Files to modify

- `src-tauri/src/task_system/processor.rs`:
  - `process_audio()` (lines 160-298)
  - `process_image()` (lines 40-135)
  - `generate_thumbnail_for_asset()` (lines 139-158)
  - Timeout constants at top of file

## Implementation plan

- [ ] Refactor `process_audio()`: load bytes outside timeout, wrap only Symphonia probing
- [ ] Refactor `process_image()`: load bytes outside timeout, wrap only image decode + thumbnail
- [ ] Refactor `generate_thumbnail_for_asset()`: same pattern
- [ ] Consider adding nested ZIP timeout distinction for image processing (currently always 30s)
- [ ] Verify existing tests still pass

## Testing notes

- Existing tests in `src-tauri/src/task_system/` should still pass
- Hard to unit test gate contention, but the timeout logic change is straightforward
