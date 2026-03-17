---
# asseteer-syol
title: Replace venv-based server startup with uv run
status: todo
type: task
priority: high
created_at: 2026-03-17T10:05:17Z
updated_at: 2026-03-17T10:06:12Z
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
