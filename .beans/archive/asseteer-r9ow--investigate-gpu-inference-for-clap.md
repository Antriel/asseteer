---
# asseteer-r9ow
title: Investigate GPU inference for CLAP
status: completed
type: task
priority: normal
created_at: 2026-03-18T11:17:39Z
updated_at: 2026-03-18T12:26:08Z
parent: asseteer-526f
---

Currently falling back to CPU despite GPU being available (GTX 1070). Could be CUDA compatibility issue — older GPU may not support newer CUDA toolkit versions, or current PyTorch wheels may not ship with compatible CUDA.

## What to investigate
- [ ] Why is CLAP currently running on CPU? Check PyTorch CUDA availability and error messages
- [ ] GTX 1070 = Pascal architecture, CUDA compute capability 6.1 — check what PyTorch/CUDA versions still support it
- [ ] Benchmark GPU vs CPU inference if we can get it working
- [ ] Consider shipping strategy: need to support various GPU configs (or graceful CPU fallback)
- [ ] Check if ONNX Runtime with DirectML/CUDA could be an easier path to GPU support across hardware


## Results (2026-03-18)

**Root cause:** uv installs CPU-only PyTorch from PyPI by default. Need `--index https://download.pytorch.org/whl/cu126` to get CUDA build.

**GTX 1070 compatibility:** cu128 dropped sm_61 (Pascal). cu126 still supports it. PyTorch 2.10.0+cu126 works.

| Mode | CPU | GPU (GTX 1070) | Speedup |
|------|-----|----------------|---------|
| Single file | 51.8ms | 25.5ms | 2.0x |
| Batch size=8 | 47.4ms | 21.4ms | 2.2x |

**Combined with 2-server approach (extrapolation for 283K files):**
- CPU single-server sequential: 51.8ms → 4.1h
- GPU single-server batch=8: 21.4ms → 1.7h
- GPU + 2-server + concurrency: could reach ~1h (estimate)

**Shipping considerations:**
- cu126 is the last CUDA variant supporting Pascal (GTX 10xx, compute 6.1)
- cu128+ requires Volta or newer (sm_70+, GTX 16xx/RTX 20xx+)
- Need runtime detection: try CUDA, fall back to CPU gracefully
- uv inline script deps need `--index` flag — can't put CUDA index in script metadata (uv 0.6.x)
