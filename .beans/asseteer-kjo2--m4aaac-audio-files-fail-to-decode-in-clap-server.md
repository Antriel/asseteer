---
# asseteer-kjo2
title: M4A/AAC audio files fail to decode in CLAP server
status: completed
type: bug
priority: high
created_at: 2026-03-18T07:28:14Z
updated_at: 2026-03-18T07:28:54Z
---

M4A files fail all three decode backends in _decode_audio_bytes: soundfile can't decode M4A (libsndfile doesn't support it), miniaudio fails on some M4A files, and the temp-file+librosa fallback fails because audioread needs ffmpeg which is not on PATH. Fix: bundle ffmpeg via imageio-ffmpeg package (ships ffmpeg binary in the wheel, downloaded as part of uv first-run setup) and add it to PATH in the server lifespan.


## Summary of Changes

- Added `imageio-ffmpeg>=0.5.1` to script dependencies (the wheel includes a pre-built ffmpeg binary, so it's downloaded by uv on first-run setup alongside other deps)
- Added `_setup_ffmpeg_path()` helper that runs at lifespan startup and adds the bundled ffmpeg dir to `PATH`
- With ffmpeg on `PATH`, librosa's audioread fallback (the third tier of `_decode_audio_bytes`) can now handle M4A/AAC files via the temp-file path
- Also fixed `UnboundLocalError` in `_decode_audio_bytes` where exception variables from earlier `except` blocks were referenced in a later one (Python 3 deletes `except ... as var` bindings when the block exits)
