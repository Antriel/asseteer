---
# asseteer-cnvq
title: Switch deflate backend to zlib-ng for SIMD-accelerated decompression
status: todo
type: task
priority: high
created_at: 2026-03-21T10:04:04Z
updated_at: 2026-03-21T10:04:04Z
parent: asseteer-k1go
---

## Problem

The zip crate currently uses `miniz_oxide` (pure Rust, no SIMD) as its deflate backend. This is the slowest option available. All ZIP decompression throughout the app — scan/import, audio processing cache fills, thumbnail extraction, asset playback — is bottlenecked by this.

## Fix

One-line change in `src-tauri/Cargo.toml`:

```toml
# Before
zip = { version = "2.2", default-features = false, features = ["deflate", "deflate64", "lzma", "bzip2", "zstd"] }

# After
zip = { version = "2.2", default-features = false, features = ["deflate-zlib-ng", "deflate64", "lzma", "bzip2", "zstd"] }
```

This switches to `zlib-ng`, a C library with runtime SIMD detection:
- x86/x64: uses AVX2/SSE2
- ARM: uses NEON
- Other: falls back to scalar code

Typical benchmarks show **2-3x faster decompression** vs miniz_oxide.

## Cross-platform notes

- Compiles from source via cmake at build time (Tauri already requires C toolchain)
- Runtime SIMD detection — no need for target-specific builds
- Widely used in production (Python 3.13+ default, used by many Rust projects)
- Works on Windows, macOS, Linux, ARM

## Current state

```
flate2 resolved features: ['any_impl', 'default', 'miniz_oxide', 'rust_backend']
```

After change, flate2 will resolve with `zlib-ng` backend instead.

## Impact

Benefits every ZIP operation in the app:
- Scan/import: nested ZIP decompression during directory enumeration (~1.3s per nested ZIP currently)
- Audio processing: ZipCache fills (decompressing 600MB-1.2GB nested ZIPs, currently 8-36 seconds)
- Thumbnail generation: extracting image bytes from ZIPs
- Asset playback/preview: loading asset bytes from ZIPs on demand

## Transitive flate2 effect

The project also depends on `flate2` directly (Cargo.toml line 59, used for tar.gz extraction of the uv binary). The `zip` crate pulls in `flate2` as well. Cargo unifies features additively — enabling `zlib-ng` via the zip crate will affect all `flate2` usage project-wide (the `zlib-ng` backend takes priority over `rust_backend`). This is fine and actually beneficial (faster tar.gz extraction too), but should be verified with a clean build and test run.

## Files to modify

- `src-tauri/Cargo.toml` — change `deflate` to `deflate-zlib-ng` in zip features (line 38)

## Verification

- [ ] `cargo build` succeeds (zlib-ng compiles from C source via cmake)
- [ ] `cargo test` passes
- [ ] Cross-compile check if CI targets multiple platforms (cmake + C compiler required)
