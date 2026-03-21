# Backend (Tauri/Rust)

## Entry Points

- `src/main.rs` → `src/lib.rs` (app setup, state init, command registration)
- Commands in `src/commands/`
- Database layer in `src/database/`
- Task system in `src/task_system/`
- CLAP semantic search in `src/clap/`

## Commands

| File | Purpose |
|------|---------|
| `scan.rs` | `add_folder` — directory walk + ZIP scanning, streams asset chunks via mpsc |
| `rescan.rs` | `preview_rescan` / `apply_rescan` — incremental diff-based folder rescan |
| `process.rs` | `start/pause/resume/stop_processing` — per-category processing orchestration |
| `folders.rs` | `list/remove/rename_folder`, `update_search_excludes` — folder CRUD |
| `assets.rs` | `get_asset_bytes` (binary IPC), `request/cancel_thumbnails` |
| `search.rs` | CLAP semantic search, server management, cache control |

## Asset Processing Pipeline

### Phase 1: Discovery (`scan.rs`)
- Recursive directory walk + ZIP archive scanning (including nested ZIPs)
- Streams chunks of 200 assets through mpsc channel
- Discovery runs on `spawn_blocking`, insertion runs async concurrently
- **Images**: PNG, JPG, JPEG, WebP, GIF, BMP
- **Audio**: MP3, WAV, OGG, FLAC, M4A, AAC

### Phase 2: Processing (`task_system/`)
- Per-category processing: Image, Audio, Clap
- Pending = assets missing metadata (determined by LEFT JOIN, not a status column)

**Images** (`processor.rs`):
- Extract dimensions via `image` crate
- Optional inline 128px WebP thumbnails (`fast_image_resize` + `webp` crate)
- ON CONFLICT preserves existing thumbnails

**Audio** (`processor.rs`):
- Extract duration/sample_rate/channels via Symphonia
- Timeout: 30s (120s for nested ZIP cache fills)

**CLAP** (`processor.rs` + `clap/`):
- Batch HTTP requests to Python CLAP server for audio embeddings
- Separate paths for filesystem files (send paths) vs ZIP files (send bytes)
- Embeddings stored as f32 BLOB in `audio_embeddings` table

### Phase 3: Rescan (`rescan.rs`)
- Two-phase: preview (diff against DB) → apply (atomic commit)
- Preview cached in `AppState` with 30-minute expiry
- Modified assets get metadata deleted for reprocessing

## Task System (`src/task_system/`)

- **`work_queue.rs`** — Crossbeam MPSC channel + tokio worker pool (CPU count - 1 workers)
  - Locality-aware batching: nested ZIP files grouped together (max 8 per batch)
  - Per-category concurrency: CLAP=1, Image/Audio=unlimited
  - Pause/resume/stop via atomic signals + generation counter for stale detection
  - Progress emitter: 2-second interval with rate/ETA calculation
- **`processor.rs`** — Actual image/audio/CLAP processing logic

## Other Modules

- **`thumbnail_worker.rs`** — Background lazy thumbnail generation (single async task + LIFO stack, semaphore-limited filesystem concurrency, sequential per ZIP)
- **`zip_cache.rs`** — Global single-item cache for decompressed nested ZIP archives. Mutex/condvar coordination so only one thread loads a given nested ZIP; parallel reads via `Arc<Vec<u8>>`
- **`utils.rs`** — Path resolution (`resolve_asset_fs_path`, `resolve_zip_path`), asset byte loading from filesystem or ZIP (with recursive nested ZIP support)
- **`models.rs`** — Core data types (`Asset`, `SourceFolder`, `ProcessingCategory`, etc.)

## Events (Tauri → Frontend)

| Event | Source |
|-------|--------|
| `scan-progress` / `rescan-progress` | Scan/rescan discovery + insertion progress |
| `processing-progress-{category}` | Processing rate, ETA, current file |
| `processing-complete-{category}` | Category finished |
| `thumbnail-ready` | Single thumbnail generated |
| `thumbnail-stats` | Thumbnail worker queue stats (2x/sec) |
| `clap-server-*` | CLAP server lifecycle events |

## Database (Write Operations)

Uses sqlx with SQLite connection pool. WAL mode, 30s busy timeout.

**Note**: All read operations are handled by frontend via Tauri SQL plugin.

## Testing

```bash
cd src-tauri && cargo test
```

- Unit tests colocated in each module (`#[cfg(test)] mod tests`)
- Shared test utilities in `test_helpers.rs` (in-memory DB, fixture builders)
- `concurrent_tests.rs` for multi-threaded scenarios
