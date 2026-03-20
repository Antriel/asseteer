---
# asseteer-q0se
title: Replace librosa with faster decoder for large WAV files
status: scrapped
type: task
priority: deferred
created_at: 2026-03-18T15:00:55Z
updated_at: 2026-03-20T09:57:32Z
parent: asseteer-526f
---

Large WAV files (~50MB) take ~100ms just for extraction + ~400ms for upload+inference. The server tries soundfile->miniaudio->ffmpeg in sequence. For WAV specifically, a direct decoder could be faster than librosa's resampling pipeline.

## Reasons for Scrapping
Parent epic scrapped — not worth extra complexity.
