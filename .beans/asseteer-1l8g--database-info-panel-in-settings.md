---
# asseteer-1l8g
title: Database info panel in Settings
status: todo
type: feature
priority: low
created_at: 2026-03-25T11:37:28Z
updated_at: 2026-03-25T11:37:28Z
---

Add a "Database" section to the Settings page with:

- **Open DB folder** button — opens the directory containing the SQLite file
- **DB size** — show current file size (main DB + WAL)
- **Stats** — anything interesting: total assets, total folders, free pages, page count, etc.
- **Vacuum button** — explicit manual VACUUM with progress/spinner. Important: temporarily enable WAL auto-checkpointing before running VACUUM to prevent duplicating the full DB size into the WAL file. Restore auto-checkpoint setting afterward.
