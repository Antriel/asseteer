---
# asseteer-fs6h
title: Add /preload endpoint to CLAP server for model pre-download
status: completed
type: task
priority: high
created_at: 2026-03-17T10:05:24Z
updated_at: 2026-03-17T10:14:33Z
parent: asseteer-5kja
---

Add endpoint that triggers HuggingFace model download without requiring an inference request.

- [ ] Add `POST /preload` endpoint to `clap_server.py`
- [ ] Endpoint should trigger model loading if not already loaded
- [ ] Return progress-friendly response (model name, size, status)
- [ ] Rust side calls `/preload` after server health check passes during first-run setup


## Summary of Changes
- Added `POST /preload` endpoint to `clap-server/clap_server.py`
- Returns model name, device, and status
- Returns 503 if model not yet loaded (server still starting)
- Added `PreloadResponse` Pydantic model
- Rust-side `/preload` call will be wired in `asseteer-syol` (uv run startup task)
