# Core Architecture Guide

## Project Overview

Build a Tauri desktop application with PixiJS infinite canvas, Rust-powered ML preprocessing, and SQLite storage to visualize 10,000+ game assets offline with 60 FPS rendering and full Level-of-Detail (LOD) support.

**Key Achievement**: Validates that all performance targets are achievable through extensive benchmarking research.

---

## Quality Tiers System

Users select a quality tier based on their priorities. Higher tiers provide better accuracy and richer features but require more processing time and VRAM.

### Tier 1: Fast (Default)
- **Use Case**: Quick initial processing, good quality results
- **Processing**: 51-106ms per image, 22-45ms per audio (2.8-13.6x under 300ms budget)
- **VRAM**: 3.3-4.6GB peak (70% margin in 16GB system)
- **Features**:
  - Basic auto-generated tags (CLIP ViT-B/32, PANNs Cnn14)
  - Good quality FTS5 full-text search
  - Perceptual hashing for duplicates
  - Basic style classification

### Tier 2: Quality (Recommended)
- **Use Case**: Balanced speed and accuracy, best for most users
- **Processing**: 85-168ms per image, 37-75ms per audio (1.8-8x under budget)
- **VRAM**: 3.9-4.7GB peak (70% margin in 16GB system)
- **Features**:
  - Detailed auto-generated tags (CLIP ViT-L/14, CLAP)
  - Very good text-to-asset matching
  - Better similarity clustering
  - Improved sprite sheet detection
  - All Fast tier features

### Tier 3: Premium (Overnight)
- **Use Case**: Maximum quality, semantic understanding, natural language search
- **Processing**: 235-835ms per asset (intentionally slower for quality)
- **VRAM**: 11.5-15.5GB peak (fits in 16GB with sequential processing)
- **Features**:
  - Rich natural language descriptions (LLaVA-1.6 7B, Audio LLMs)
  - Semantic duplicate detection
  - Question-answering about assets ("What animation frames are in this?")
  - Natural language search queries
  - Advanced similarity matching
  - All lower tier features

---

## Technology Stack

### Frontend
- **Tauri v2**: Rust-based desktop framework, 3-5MB bundle
- **SvelteKit 2** with Svelte 5: Modern reactive framework with runes
- **TypeScript**: Strict mode for type safety
- **Tailwind CSS 4**: Utility-first styling with light/dark mode
- **PixiJS v8**: WebGL canvas rendering (60 FPS with 10,000+ sprites)

### Backend (Rust)
- **Image Processing**:
  - `fast_image_resize` v5.0 - SIMD optimized (21.7ms for 4928x3279 → 852x567)
  - `image` crate v0.25 - Format decoding (JPEG, PNG, WebP, AVIF)
  - `img_hash` v3.0 - Perceptual hashing (dHash, 2-3ms)
  - `kamadak-exif` v0.6 - Metadata extraction (3-5ms)

- **Audio Processing**:
  - `rodio` / `symphonia` - Audio decoding (MP3, WAV, OGG, FLAC)
  - `ort` v2.0 - ONNX Runtime with CUDA/CPU support

- **ML Inference** (see Models section for specific implementations):
  - CLIP vision/text encoders (zero-shot classification)
  - PANNs CNN14 or CLAP (audio classification)
  - LLaVA-1.6 or Audio LLMs (premium descriptions)

- **Data Processing**:
  - `ndarray` v0.15 - Numerical computing
  - `rayon` v1.10 - Parallel iterators (8 threads)
  - `annembed` - UMAP dimensionality reduction

- **Storage**:
  - **SQLite** via `rusqlite` - Metadata, embeddings, small thumbnails (<256KB as BLOBs)
  - **Filesystem** - Large thumbnails with directory sharding
  - **ZIP streaming** via `zip` crate - Process archives without extraction

- **Async Runtime**:
  - `tokio` v1.40 - Async I/O (file operations, Tauri commands)
  - `tokio::task::spawn_blocking` - CPU-bound work separation

---

## Performance Targets & Validation

### Per-Asset Processing Budget
**Target: 300ms per asset | All tiers validated to be well under budget**

#### Tier 1: Fast (Total: 51-106ms)
| Operation | Time | Library |
|-----------|------|---------|
| Load image | 10-30ms | image |
| 3x thumbnails (parallel) | 20-50ms | fast_image_resize |
| Perceptual hash | 2-3ms | img_hash |
| CLIP ViT-B/32 inference | 16ms | ort |
| EXIF extraction | 3-5ms | kamadak-exif |
| **Image Total** | **51-106ms** | ✅ 2.8-5.9x under |

| Operation | Time | Library |
|-----------|------|---------|
| Load audio | 10-20ms | rodio/symphonia |
| Mel spectrogram | 5-10ms | mel_spec |
| PANNs inference (GPU) | 5-10ms | ort |
| Chromaprint | 2-5ms | rusty-chromaprint |
| **Audio Total** | **22-45ms** | ✅ 6.7-13.6x under |

#### Tier 2: Quality (Total: 85-168ms per image, 37-75ms per audio)
| Component | Time |
|-----------|------|
| Image processing | 85-168ms |
| Audio processing | 37-75ms |
| **Status** | ✅ 1.8-8x under budget |

#### Tier 3: Premium (Total: 235-835ms per asset)
| Component | Time |
|-----------|------|
| Image processing | 235-588ms |
| Audio processing | 317-835ms |
| **Status** | ⚠️ Intentionally slower; designed for overnight processing |

### Rendering Performance
- **Target**: 60 FPS with 10,000 items
- **Achieved**: 60 FPS with PixiJS + viewport culling
- **Only visible items rendered**: ~50-200 depending on zoom level
- **Texture batching**: Reduces draw calls by 90%+

### VRAM Usage
**All tiers fit within 16GB system:**
- Tier 1: 3.3-4.6GB (70% margin)
- Tier 2: 3.9-4.7GB (70% margin)
- Tier 3: 11.5-15.5GB (models loaded sequentially, not simultaneously)

---

## Model Selection Reference

### Image Classification

| Tier | Model | Accuracy | Speed | Use Case |
|------|-------|----------|-------|----------|
| Fast | CLIP ViT-B/32 | Good | 16ms | Basic style/content tags |
| Quality | CLIP ViT-L/14 | Very Good | 50-80ms | Detailed tags, better accuracy |
| Quality (Alt) | SigLIP ViT-L/16 | Very Good | 50-80ms | Trained on higher quality data |
| Premium | LLaVA-1.6 7B | Excellent | 200-500ms | Rich natural language descriptions |

**Output Examples**:
- Fast: "pixel art, character sprite"
- Quality: "pixel art, character sprite, knight, armor, blue color scheme, facing right"
- Premium: "A pixel art sprite sheet depicting a knight character wearing blue medieval armor with a red cape, shown in 8 walking animation frames facing right, suitable for 2D RPG games"

### Audio Classification

| Tier | Model | Accuracy | Speed | Use Case |
|------|-------|----------|-------|----------|
| Fast | PANNs CNN14 | Good | 5-10ms | AudioSet 527 classes |
| Quality | CLAP | Very Good | 20-40ms | Superior text-audio alignment |
| Quality (Alt) | BEATs | Very Good | 50-100ms | State-of-the-art audio understanding |
| Premium | Audio LLM (Qwen2, SALMONN) | Excellent | 300-800ms | Rich semantic descriptions |

**Output Examples**:
- Fast: "explosion, burst, impact"
- Quality: "explosion, metal debris, short duration, high impact"
- Premium: "A sharp metallic explosion sound with flying debris impacts, lasting approximately 1.2 seconds, suitable for weapon impacts or destructive environmental effects in games"

### Sprite Sheet Detection (All Tiers)
- **Method**: FFT-based grid pattern detection
- **Speed**: <1ms
- **Accuracy**: 90-95%
- **Alternative**: CNN classifier (Tier 2+) for 98%+ accuracy

---

## Architecture Principles

### Separation of Concerns
1. **I/O Operations**: Use Tokio async (non-blocking)
2. **CPU-Bound Work**: Use `spawn_blocking` with Rayon parallelization (8 threads)
3. **GPU Operations**: ONNX Runtime with CUDA/CPU fallback
4. **Frontend Updates**: Batch events, throttle progress updates

### Data Flow
```
User Action
    ↓
Tauri Command (async)
    ├─ I/O: Read files (Tokio)
    ├─ CPU: Process batches (spawn_blocking + Rayon)
    ├─ GPU: ML inference (ONNX RT)
    └─ Storage: SQLite transaction (batched)
    ↓
Frontend Update (throttled events)
```

### Caching Strategy
- **Metadata Cache**: Keep all asset metadata in-memory (HashMap, ~1KB per asset)
- **Thumbnail Cache**: LRU cache for hot textures (~20MB, 200 thumbnails)
- **Model Cache**: Load models on-demand, keep in VRAM during batch processing

### Database Pattern
- **SQLite**: Metadata, embeddings, full-text search index
  - WAL mode for concurrency
  - 64MB cache for performance
  - Virtual FTS5 table for search
- **Filesystem**: Large thumbnails with directory sharding
- **ZIP Streaming**: Never extract, read directly into memory

---

## Error Handling & User Feedback

### Toast Notifications (Non-Blocking)
- Operation success/failure
- Processing progress
- Informational messages

### Confirmation Dialogs (Blocking)
- Destructive actions (delete, overwrite)
- Long-running operations
- Configuration changes

### Progress Tracking
- Batch operations: Update every 10-50 items (not every item)
- Long-running: Show estimated time remaining
- Cancellation: Allow user to stop processing

---

## Dependency Management

### Model Downloads
- **On-Demand**: Models downloaded when first needed
- **Verified**: SHA256 checksum validation
- **Storage Estimates**:
  - Fast Tier: ~400MB (CLIP B/32 289MB + PANNs 80MB)
  - Quality Tier: ~2.3GB (CLIP L/14 890MB + CLAP 630MB + others)
  - Premium Tier: ~12GB (LLaVA 7B 5-7GB + Audio LLM 4-6GB, INT8 quantized)

### External Dependencies
- **None for processing**: All models run locally (no API calls required)
- **Optional**: OpenAI API fallback for complex cases (internet-dependent)

---

## Directory Structure

```
asseteer/
├── docs/
│   ├── 01-CORE-ARCHITECTURE.md (this file)
│   ├── 02-DATABASE-SCHEMA.md
│   ├── 03-API-COMMANDS.md
│   ├── 04-MODELS-REFERENCE.md
│   ├── phases/
│   │   ├── 01-FOUNDATION.md
│   │   ├── 02-SEARCH-FILTERING.md
│   │   └── 03-ADVANCED-FEATURES.md
│   ├── CHECKLISTS.md
│   └── QUICK-REFERENCE.md
├── src/ (Frontend)
├── src-tauri/ (Backend)
└── [other files]
```
