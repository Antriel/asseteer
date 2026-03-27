---
# asseteer-jtbv
title: 'ZIP audio playback: binary IPC for get_asset_bytes'
status: completed
type: task
priority: normal
created_at: 2026-03-20T07:44:28Z
updated_at: 2026-03-20T07:46:20Z
parent: asseteer-kvnt
---

get_asset_bytes returns Vec<u8> which Tauri serializes as a JSON number[] array. A 10MB WAV becomes ~40MB of JSON. Switch to tauri::ipc::Response on the Rust side and ArrayBuffer on the frontend to skip JSON serialization entirely.

## Summary of Changes

- `commands/assets.rs`: changed return type from `Result<Vec<u8>, String>` to `Result<Response, String>` using `tauri::ipc::Response::new()`, so Tauri sends `InvokeResponseBody::Raw` (binary) instead of serializing as a JSON number array
- `AudioPlayer.svelte`: updated `invoke<number[]>` to `invoke<ArrayBuffer>` and removed the `new Uint8Array(bytes)` wrapper since `Blob` accepts `ArrayBuffer` directly
