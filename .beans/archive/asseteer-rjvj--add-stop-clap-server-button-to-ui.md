---
# asseteer-rjvj
title: Add Stop CLAP Server button to UI
status: completed
type: feature
priority: normal
created_at: 2026-04-09T15:25:02Z
updated_at: 2026-09-25T12:38:37Z
---

There's no UI to stop the CLAP server once it's running. Users can only clear cache, which fails because the server is still using it. Add a Stop Server action to the CLAP settings section.

## Summary of Changes

- New `stop_clap_server` command, which stops the whole uv→python tree and waits for it to exit (see asseteer-vayv). Also `stopClapServer()` in `queries.ts` and `clapState.stopServer()`, which sets the status to offline and stops the health monitor.
- Settings, Ready state: a "Stop Server" button with a "Stopping..." spinner and the hint "Starts again on the next semantic search." The button is disabled while CLAP processing is running, because processing would just restart the server.
- Checked in the harness (both themes): ready → stopping → offline, and clear → "Not set up" with the fake cache files deleted. No console errors.
