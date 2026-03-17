---
# asseteer-12yf
title: uv binary download and management in Rust
status: todo
type: task
priority: high
created_at: 2026-03-17T10:05:12Z
updated_at: 2026-03-17T10:05:12Z
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
