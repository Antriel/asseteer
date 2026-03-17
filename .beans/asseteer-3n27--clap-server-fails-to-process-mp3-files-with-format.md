---
# asseteer-3n27
title: CLAP server fails to process MP3 files with 'Format not recognised'
status: completed
type: bug
priority: high
created_at: 2026-03-17T11:41:44Z
updated_at: 2026-03-17T12:04:23Z
---

## Symptom

CLAP server throws `soundfile.LibsndfileError: Error opening <_io.BytesIO object at 0x...>: Format not recognised` when processing MP3 files.

Example from log (`clap-server-2026-03-17T12-16-41.log`):
```
RuntimeError: Failed to decode audio file 'Footsteps-forest-11.mp3' (.mp3): Error opening <_io.BytesIO object at 0x00000185BE922070>: Format not recognised.
```

Test file observed in logs: **Footsteps-forest-11.mp3** (from Wild West Sound FX Pack or similar).

## Root Cause (pre-uv investigation)

The error is **not** an MP3 compatibility issue — librosa does support MP3. The real cause was a **Python version ABI mismatch** in the old venv:

- The venv ran Python 3.14 but installed wheels were compiled for Python 3.13 (`cp313` tags)
- `soundfile` couldn't initialize at all (`No module named '_cffi_backend'`)
- This caused failure on **all** audio formats, not just MP3 — MP3 was just the first file hit
- librosa's `audioread` fallback also failed (ffmpeg not on PATH)

The fix at the time was to rebuild the venv so correct `cp314` wheels would be installed.

## Current Status

We've since switched from manual venv to **uv-managed environments**. The bug is still present (same error seen in today's logs). It's unknown whether:
- uv is selecting the wrong Python version or wrong wheel tags
- The uv environment has the same ABI mismatch
- ffmpeg availability is still an issue for the audioread fallback

## What to Investigate

- [ ] Check which Python version uv is using for the clap-server environment
- [ ] Verify that installed wheels (soundfile, numpy, cffi) match that Python version
- [ ] Test processing a known MP3 file end-to-end (e.g. `Footsteps-forest-11.mp3`) and confirm it works or get a clear error
- [ ] Check if ffmpeg is on PATH as a fallback option


## Fix (2026-03-17)

**Root cause confirmed**: libsndfile's virtual IO interface has no way to detect format from content (no magic byte sniffing) — relies on file extension. MP3 via BytesIO fails even in libsndfile 1.2.2 because the mpg123 decoder has additional constraints in the virtual IO path. File path works fine because libsndfile opens it natively via the OS.

**Fix**: Added `miniaudio>=1.2` as a fallback decoder in `clap_server.py`. `_process_audio_bytes` now tries soundfile/librosa first, and falls back to miniaudio if it fails. miniaudio decodes MP3 (and other formats) directly from bytes with no external dependencies.

- Added `miniaudio>=1.2` to script dependencies header in `clap_server.py`
- Extracted `_decode_audio_bytes()` helper with try/fallback logic
- Tested with `Footsteps-forest-01.mp3` — soundfile fails, miniaudio succeeds (1.15s, 55171 samples at 48kHz)

**Note**: The uv environment will automatically pick up miniaudio on next first-run setup since it's in the script dependencies.
