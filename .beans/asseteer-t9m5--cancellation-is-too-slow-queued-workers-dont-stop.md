---
# asseteer-t9m5
title: Cancellation is too slow — queued workers don't stop
status: todo
type: bug
priority: normal
created_at: 2026-03-26T14:50:19Z
updated_at: 2026-03-26T15:00:18Z
---

When stopping processing (especially CLAP), cancellation takes way too long. Root cause appears to be: we fill all workers but run them at concurrency of 2. When cancel is requested, the already-queued workers that haven't started yet don't get cancelled — so if each worker processes a batch of 16 items and takes ~6 seconds, we wait for many workers to finish before the stop actually takes effect.

## Notes from CLAP ZIP investigation (2026-03-26)

Current CLAP instrumentation shows the slow part is Rust-side prep before the Python request, not Python inference:
- regular ZIP CLAP batches spend ~6-9s in `prep_ms`
- the Python batch upload itself is only ~0.3-0.4s
- slow batches observed so far all had `distinct_zip_files=1`, so outer-ZIP locality is already good

This matters for cancellation because workers can spend several seconds inside ZIP prep before reaching a request boundary. If we move CLAP to the staged regular-ZIP dispatcher pattern (see `asseteer-mmwn`), we should review whether cancellation can stop:
- queued but not yet forwarded staged batches
- extractor threads blocked on `blocking_send`
- already-buffered `preloaded_bytes` work

The likely fix path for throughput and cancellation should be coordinated so we do not improve feed rate while keeping long stop latency.
