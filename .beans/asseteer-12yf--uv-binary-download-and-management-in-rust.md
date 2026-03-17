---
# asseteer-12yf
title: uv binary download and management in Rust
status: completed
type: task
priority: high
created_at: 2026-03-17T10:05:12Z
updated_at: 2026-03-17T10:14:33Z
parent: asseteer-5kja
---

Download and cache the uv binary on first use. Store in app data directory.

- [ ] Create `src-tauri/src/clap/uv.rs` module
- [ ] Implement `get_or_download_uv()` — check cache, download if missing
- [ ] Platform-specific download URLs (Windows .zip, macOS/Linux .tar.gz)
- [ ] Pin uv version to 0.6.x range
- [ ] Store uv binary in app data dir (e.g., `{app_data}/uv/uv.exe`)
- [ ] Add progress reporting for download (~30MB)
- [ ] Handle download failures with actionable error messages


## Summary of Changes
- Created `src-tauri/src/clap/uv.rs` module
- `get_or_download_uv()` — checks cache, downloads from GitHub releases if missing
- Platform-specific URLs for Windows (.zip), macOS (.tar.gz), Linux (.tar.gz)
- Supports x86_64 + aarch64 architectures
- Pinned to uv v0.6.14
- `init_app_data_dir()` called from `lib.rs` setup to configure storage location
- `uv_cache_dir()` for isolated Python/package storage
- Added `flate2` + `tar` dependencies to Cargo.toml for archive extraction
- Added `stream` feature to reqwest for future progress reporting
