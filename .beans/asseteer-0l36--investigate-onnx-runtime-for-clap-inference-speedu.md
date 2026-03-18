---
# asseteer-0l36
title: Investigate ONNX Runtime for CLAP inference speedup
status: todo
type: task
priority: deferred
created_at: 2026-03-18T11:17:34Z
updated_at: 2026-03-18T12:31:37Z
parent: asseteer-526f
---

CLAP model inference is 34ms/file (66% of in-process time). ONNX Runtime could potentially be 2-4x faster on CPU. Also investigate whether there are better/faster audio embedding models available now.

## Models to evaluate
- Current: CLAP (LAION-CLAP)
- M2D-CLAP — SOTA on general audio + music tasks (AudioSet mAP 49.0)
- MuQ-MuLan — better music similarity than LAION-CLAP (ROC-AUC 79.3 vs 73.9 on MagnaTagATune)
- ELSA — +2.8% R@1 over LAION-CLAP baseline, adds spatial awareness
- Diverse Audio Embeddings — reported to outperform CLAP

## What to check
- [ ] Can current CLAP model export to ONNX? Benchmark inference speed difference
- [ ] Are any of the above models available with pretrained weights and easy to integrate?
- [ ] Do alternatives support text-to-audio similarity (required for our search feature)?
- [ ] Quality comparison: do alternatives produce better embeddings for SFX similarity search?

## Model Evaluation Summary

**Recommendation: Stay with LAION-CLAP (MIT license) for now.**

- **M2D-CLAP** — Best general-audio performance (AudioSet mAP 49.0, strong SFX/env sounds, not music-biased). Weights available from nttcslab/m2d. **However: non-commercial license only (LICENSE.pdf)**. Skip unless use is strictly non-commercial.
- **MuQ-MuLan** — Skip. Music-specific (MagnaTagATune SOTA), would underperform on general SFX/environmental sounds.
- **ELSA** — Promising (+2.8% R@1 over LAION-CLAP, adds spatial/directional audio queries). **No public weights** (Apple NeurIPS 2024 paper only). Watch for future release.
- **Diverse Audio Embeddings** — Skip. Not a text-audio model; no contrastive language alignment, so no semantic search support.
- **MS-CLAP** — Viable alternative to LAION-CLAP; MIT/permissive license, competitive retrieval performance. Worth benchmarking.

**If M2D-CLAP performance is needed commercially**: contact NTT authors about licensing, or wait for a permissive-license SOTA model (field moves fast, likely by mid-2026).
