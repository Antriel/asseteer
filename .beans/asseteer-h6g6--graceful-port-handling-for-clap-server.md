---
# asseteer-h6g6
title: Graceful port handling for CLAP server
status: completed
type: task
priority: low
created_at: 2026-03-17T10:05:45Z
updated_at: 2026-03-20T10:12:35Z
parent: asseteer-5kja
blocked_by:
    - asseteer-syol
---

Handle port conflicts instead of failing silently.

- [x] Try ports 5555, 5556, 5557 in sequence if port is in use
- [x] Store active port in app state so client knows where to connect
- [x] Update client.rs to use dynamic port instead of hardcoded 5555
- [x] Show which port is in use in settings/status

## Summary of Changes

- **`clap_server.py`**: Added `--port` CLI argument (default 5555) via argparse, passed to `uvicorn.run()`
- **`client.rs`**: Replaced hardcoded `CLAP_SERVER_URL` constant with `AtomicU16 ACTIVE_PORT` (default 5555). `ClapClient` now computes base URL dynamically via `base_url()` method reading from global. Added `set_active_port()` / `get_active_port()`. Added `port: u16` field to `HealthInfo` (populated after deserialization from global).
- **`server.rs`**: Added `find_free_port()` — tries 5555, 5556, 5557 via `TcpListener::bind`. Calls `set_active_port()` before starting process. Passes `--port <N>` to both uv run and venv fallback startup paths.
- **Frontend**: `ClapServerInfo` gains `port: number`. `ClapState` tracks `port`. StatusBar shows port number when not on default 5555.
