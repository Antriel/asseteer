---
# asseteer-mxsj
title: Benchmark nested ZIP processing performance
status: completed
type: task
priority: normal
created_at: 2026-03-18T11:17:26Z
updated_at: 2026-03-18T15:00:30Z
parent: asseteer-526f
---

Nested ZIPs (ZIP-inside-ZIP) can't seek into the inner archive without decompressing the outer entry first. Each file access likely decompresses the entire inner ZIP into memory. Benchmark to quantify actual cost, then design a caching strategy (hold decompressed inner ZIP in memory/tempfile for consecutive accesses).

## Benchmark Results (2026-03-18, GPU/CUDA)

### Small SFX files (Wild West, ~2MB WAVs)
- Filesystem path: ~50ms/file (~30 files/sec)
- ZIP upload (cached handle): ~35ms/file
- ZIP reopen per file: ~33ms/file (ZIP overhead ~1.5ms, negligible)
- Concurrent (2 workers): best at ~38 files/sec

### Large music files (nested ZIP, ~50MB WAVs)
- Naive (reopen all per file): 791ms/file (outer=249ms + extract=110ms + upload=432ms)
- Cached inner ZIP: 585ms/file (26% faster, saves ~206ms/file)
- Upload+inference dominates at 422ms for large files
- Caching inner ZIP avoids re-reading ~723MB from outer each time

### Key findings
- GPU makes small-file inference so fast that ZIP overhead is negligible
- For nested ZIPs, caching decompressed inner ZIP is the biggest easy win
- Batch endpoint is broken (JSON serialization issue with Windows paths)
- Large files are ~10x slower per file than small SFX due to librosa loading + larger tensors


## Updated Results (2026-03-18, GPU/CUDA, batch endpoint fixed)

### Small SFX files (~2MB WAVs) - 30 files
| Method | Per file | Throughput | vs Single |
|--------|----------|------------|-----------|
| Single (filesystem) | 44ms | 25.7/sec | 1.0x |
| Single (ZIP reopen) | 43ms | - | ~same |
| Batch (size=8) | 23ms | 41.8/sec | 1.9x |
| Concurrent singles (2 workers) | 25ms | 40.8/sec | 1.8x |
| **Concurrent batches (8, 2 workers)** | **18ms** | **54.9/sec** | **2.4x** |

### Large music files (nested ZIP, ~50MB WAVs) - 16 files
| Method | Per file | Notes |
|--------|----------|-------|
| Naive (reopen all) | 842ms | outer=228ms + extract=107ms + upload=507ms |
| Cached inner ZIP | 604ms | 28% faster, caching saves ~238ms/file |
| Cached + batch upload (4) | 626ms | Batching doesn't help much for large files |
| Cached + batch upload (8) | 624ms | Same - upload dominates, not inference |

### Key findings
- Batch endpoint was broken (padding=True conflicted with ClapFeatureExtractor)
- For small files: batch size=8 with 2 concurrent workers is optimal (2.4x speedup, ~1.4h for 283K files)
- For large files: batching barely helps because audio decoding (librosa) dominates, not GPU inference
- For large nested ZIPs: caching inner ZIP is the main win (28%)
- Memory concern: each inner ZIP decompressed = ~723MB in memory. With caching, only 1 at a time


## Summary of Changes

### Server-side (clap_server.py)
- Fixed batch endpoint: removed `padding=True` from processor call (conflicted with ClapFeatureExtractor's internal padding)
- Added non-Tensor output check to shared `_batch_encode()` function
- Added error logging + detail messages for batch inference failures

### Rust-side
- **Batch CLAP inference** (`processor.rs`): New `process_clap_embedding_batch()` processes up to 8 assets per request. Filesystem files use `/embed/audio/batch`, ZIP files use `/embed/audio/batch/upload`.
- **Inner ZIP caching** (`processor.rs`): `load_asset_bytes_cached()` keeps decompressed inner ZIP in memory across consecutive files from the same nested ZIP, avoiding ~250ms re-read per file.
- **Batch client methods** (`client.rs`): Added `embed_audio_batch_paths()` and `embed_audio_batch_bytes()`.
- **Work queue batching** (`work_queue.rs`): CLAP worker collects up to 8 items from queue before sending a single batch request.

### Benchmarking
- Added `benchmark_nested.py` for nested ZIP benchmarks
- Fixed Unicode encoding error in `benchmark.py` summary output
- Added uv script metadata to `benchmark.py`
