---
# asseteer-dylo
title: Add PEP 723 inline dependency metadata to clap_server.py
status: completed
type: task
priority: high
created_at: 2026-03-17T10:05:11Z
updated_at: 2026-03-17T10:14:33Z
parent: asseteer-5kja
blocked_by:
    - asseteer-e525
---

Add inline script metadata (PEP 723) so uv can resolve dependencies without requirements.txt.

- [ ] Add `# /// script` block with `requires-python = ">=3.11,<3.14"` and all dependencies
- [ ] Test that `uv run clap_server.py` works with inline metadata
- [ ] Keep `requirements.txt` for manual fallback users


## Summary of Changes
- Added PEP 723 inline script metadata to `clap-server/clap_server.py`
- Pins `requires-python = ">=3.11,<3.14"`
- Declares all 8 dependencies (torch, transformers, librosa, soundfile, numpy, fastapi, uvicorn, python-multipart)
- Kept `requirements.txt` for manual fallback users
- Updated docstring to mention `uv run` as primary run method
