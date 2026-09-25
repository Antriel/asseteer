---
# asseteer-sous
title: 'Linux build: drag needs GTK window'
status: completed
type: bug
created_at: 2026-09-25T12:51:24Z
updated_at: 2026-09-25T12:51:24Z
---

CI (ubuntu) failed: drag::start_drag takes &gtk::ApplicationWindow on Linux, we passed &tauri::Window.

## Summary of Changes

On Linux, start_asset_drag now resolves window.gtk_window() on the main thread and passes that to drag::start_drag; failure is returned as a drag error. Windows/macOS unchanged.
