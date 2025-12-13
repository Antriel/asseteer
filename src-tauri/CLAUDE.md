# Backend (Tauri/Rust)

## Entry Points

- `src/main.rs` → `src/lib.rs`
- Commands in `src/commands/`
- Database layer in `src/database/`

## Commands

| File | Purpose |
|------|---------|
| `scan.rs` | Asset discovery, file scanning (writes to DB) |
| `process.rs` | Asset processing pipeline + worker management |

**Exposed to frontend:**
```rust
start_scan(root_path: String) -> Result<(), String>
start_processing_assets() -> Result<(), String>
pause_processing() / resume_processing() / stop_processing()
get_processing_progress() -> Result<ProcessingProgress, String>
```

## Asset Processing Pipeline

### Phase 1: Discovery (`scan.rs`)
- Recursive directory scan for supported formats
- Inserts with `processing_status = 'pending'`
- **Images**: PNG, JPG, JPEG, WebP, GIF, BMP
- **Audio**: MP3, WAV, OGG, FLAC, M4A, AAC

### Phase 2: Processing (`process.rs`)

**Images**:
- Extract dimensions
- Generate 128px JPEG thumbnails (`fast_image_resize`, Lanczos3)
- Store thumbnail as BLOB (<20KB)
- Rayon parallel batches (50 files/batch)

**Audio**:
- Extract metadata via Symphonia (duration, sample rate, channels)
- No thumbnails

### Progress & Events
- Emits `process-progress` events during processing
- Frontend listens via `@tauri-apps/api/event`

### Status Updates
- Success: `processing_status = 'complete'` + metadata/thumbnail
- Error: `processing_status = 'error'` + error message

## Task System (`src/task_system/`)

- `work_queue.rs` - Async worker pool (4 parallel workers)
- `processor.rs` - Image/audio processing logic

## Database (Write Operations)

Uses sqlx with connection pooling. Transactions for atomic batch updates.

```rust
sqlx::query("INSERT INTO assets (...) VALUES (...)")
    .bind(...)
    .execute(&pool)
    .await?;
```

**Note**: All read operations are handled by frontend via Tauri SQL plugin.

## Testing

```bash
cd src-tauri && cargo test
```

- Unit tests for pure functions
- Integration tests in `tests/`
