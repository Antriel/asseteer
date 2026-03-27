---
# asseteer-syol
title: Replace venv-based server startup with uv run
status: completed
type: task
priority: high
created_at: 2026-03-17T10:05:17Z
updated_at: 2026-03-17T10:16:45Z
parent: asseteer-5kja
blocked_by:
    - asseteer-12yf
    - asseteer-dylo
    - asseteer-fs6h
---

Replace the current venv Python spawning in server.rs with uv run --python 3.13.

- [ ] Modify `ensure_server_running()` to use uv binary instead of venv python
- [ ] Pass `--python 3.13` to pin Python version
- [ ] Set `UV_CACHE_DIR` to app data dir for clean isolation
- [ ] Keep manual venv fallback: if user has configured a custom python path, use that instead
- [ ] Increase startup timeout from 60s to 120s (first run downloads Python + deps)
- [ ] Add `/preload` endpoint call after server starts to trigger model download
- [ ] Update error messages to be actionable (no more "run pip install" instructions)


## Summary of Changes

### server.rs — full rewrite
- **Primary startup**: `uv run --python 3.13 clap_server.py` with `UV_CACHE_DIR` set to app data
- **Fallback**: If uv download fails and a manual venv exists, uses venv python directly
- **Timeout**: Increased from 60s to 120s (240 × 500ms polls) for first-run downloads
- **Preload**: Calls `POST /preload` after health check passes to ensure model is loaded
- **Error messages**: Actionable — tells user what to do (restart app, check internet, see README)
- **Refactored** into focused functions: `find_clap_server_dir()`, `start_server_process()`, `start_server_venv_fallback()`, `wait_for_server_ready()`, `call_preload()`

### client.rs
- Added `preload()` method — POST to `/preload` endpoint

### Compilation
- Zero warnings, clean build
