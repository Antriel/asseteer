---
# asseteer-qin7
title: Manual fallback setup scripts (setup.bat/setup.sh)
status: scrapped
type: task
priority: low
created_at: 2026-03-17T10:05:55Z
updated_at: 2026-03-20T10:53:38Z
parent: asseteer-5kja
---

Provide setup scripts for users who prefer to manage their own Python environment.

- [ ] Create `clap-server/setup.bat` with venv creation, ABI mismatch detection, pip install
- [ ] Create `clap-server/setup.sh` for Unix with same logic
- [ ] Add stale venv detection (Python version change → rebuild)
- [ ] Document both uv (automatic) and manual approaches in README
