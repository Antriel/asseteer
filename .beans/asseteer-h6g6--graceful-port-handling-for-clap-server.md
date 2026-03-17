---
# asseteer-h6g6
title: Graceful port handling for CLAP server
status: todo
type: task
priority: low
created_at: 2026-03-17T10:05:45Z
updated_at: 2026-03-17T10:06:12Z
parent: asseteer-5kja
blocked_by:
    - asseteer-syol
---

Handle port conflicts instead of failing silently.

- [ ] Try ports 5555, 5556, 5557 in sequence if port is in use
- [ ] Store active port in app state so client knows where to connect
- [ ] Update client.rs to use dynamic port instead of hardcoded 5555
- [ ] Show which port is in use in settings/status
