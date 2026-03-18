---
# asseteer-2mjx
title: 'Pipeline I/O: extract next batch while current processes'
status: todo
type: task
priority: deferred
created_at: 2026-03-18T15:00:49Z
updated_at: 2026-03-18T15:00:49Z
parent: asseteer-526f
---

Extract the next batch of audio files from ZIP while the current batch is being processed by CLAP server. Could overlap ~100ms extraction time with ~400ms inference time for large files.
