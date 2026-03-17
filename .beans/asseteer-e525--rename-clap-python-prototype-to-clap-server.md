---
# asseteer-e525
title: Rename clap-python-prototype to clap-server
status: completed
type: task
priority: high
created_at: 2026-03-17T10:05:05Z
updated_at: 2026-03-17T10:14:33Z
parent: asseteer-5kja
---

Rename the directory and update all references throughout the codebase.

- [ ] Rename `clap-python-prototype/` directory to `clap-server/`
- [ ] Update path discovery in `src-tauri/src/clap/server.rs`
- [ ] Update any references in docs, README, CLAUDE.md files
- [ ] Verify server still starts correctly after rename


## Summary of Changes
- Renamed `clap-python-prototype/` directory to `clap-server/` via `git mv`
- Updated path discovery in `src-tauri/src/clap/server.rs` (3 references)
- Updated error message in `server.rs`
- Updated `benchmark.py` reference
