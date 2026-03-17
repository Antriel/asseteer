---
# asseteer-fs6h
title: Add /preload endpoint to CLAP server for model pre-download
status: todo
type: task
priority: high
created_at: 2026-03-17T10:05:24Z
updated_at: 2026-03-17T10:05:24Z
parent: asseteer-5kja
---

Add endpoint that triggers HuggingFace model download without requiring an inference request.

- [ ] Add `POST /preload` endpoint to `clap_server.py`
- [ ] Endpoint should trigger model loading if not already loaded
- [ ] Return progress-friendly response (model name, size, status)
- [ ] Rust side calls `/preload` after server health check passes during first-run setup
