---
# asseteer-l2m5
title: 'ZIP audio playback: serve via custom Tauri URI protocol'
status: scrapped
type: task
priority: normal
created_at: 2026-03-20T07:44:28Z
updated_at: 2026-03-20T07:53:21Z
parent: asseteer-kvnt
---

Register a zipasset:// Tauri protocol so <audio> streams directly from the protocol handler (DB lookup + ZIP extraction). Eliminates blob URL management, IPC round-trip, and enables range requests.

## Reasons for Scrapping

After switching get_asset_bytes to binary IPC (ArrayBuffer), playback starts essentially immediately after the Rust-side load completes. The IPC transfer overhead is no longer a bottleneck even for 30MB WAVs. A custom protocol would add significant complexity for no perceptible gain.
