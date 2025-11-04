# Asset Manager MVP Implementation Plan

## Overview

Build a **Tauri + Svelte desktop application** for offline game asset management with **basic table/grid view** and **fast text search**. Focus on core functionality: scan assets, extract metadata, enable search/filter, display results.

**MVP Scope:**
- Table/grid display of 10,000+ assets
- Fast text search with basic filters
- Images and audio support
- Minimal preprocessing (thumbnails + basic metadata)
- Pause/resume scanning with progress tracking
- Small database footprint

**Future Expansion Ready:**
- ML-powered tagging (Fast/Quality/Premium tiers)
- Infinite canvas visualization
- Advanced similarity search
- Duplicate detection

---

## Tech Stack

### Frontend
- **SvelteKit 2** (Svelte 5 runes) + TypeScript
- **Tailwind CSS 4** for styling
- **Vite 6** as build tool
- Virtual scrolling for large lists (react-window or svelte-virtual)

### Backend
- **Tauri 2** (Rust)
- **SQLite** (rusqlite) with FTS5 for search
- **image crate** for image processing
- **rodio/symphonia** for audio metadata
- **fast_image_resize** for thumbnails
- **tokio** for async I/O
- **rayon** for parallel processing

---

## Database Schema (MVP)

**Design Goals:**
- Minimal file size (normalize data, avoid duplication)
- Fast full-text search (FTS5)
- Extensible for future ML features

```sql
-- Core assets table
CREATE TABLE assets (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    path TEXT NOT NULL,              -- Full path or ZIP path
    zip_entry TEXT,                  -- Entry name if in ZIP
    asset_type TEXT NOT NULL,        -- 'image' or 'audio'
    format TEXT NOT NULL,            -- 'png', 'jpg', 'mp3', etc.
    file_size INTEGER NOT NULL,

    -- Image metadata
    width INTEGER,
    height INTEGER,

    -- Audio metadata
    duration_ms INTEGER,
    sample_rate INTEGER,
    channels INTEGER,

    -- Thumbnail (small only for MVP)
    thumbnail_data BLOB,             -- <128KB stored inline

    -- Timestamps
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,

    -- Processing state
    processing_status TEXT DEFAULT 'pending',  -- 'pending', 'processing', 'complete', 'error'
    processing_error TEXT
);

-- Indexes for performance
CREATE INDEX idx_assets_type ON assets(asset_type);
CREATE INDEX idx_assets_path ON assets(path);
CREATE INDEX idx_assets_status ON assets(processing_status);
CREATE INDEX idx_assets_modified ON assets(modified_at);

-- Full-text search (FTS5)
CREATE VIRTUAL TABLE assets_fts USING fts5(
    filename,
    path_segments,               -- Normalized path parts
    content=assets,
    content_rowid=id,
    tokenize='porter unicode61 remove_diacritics 1'
);

-- Triggers to sync FTS
CREATE TRIGGER assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, filename, path_segments)
    VALUES (new.id, new.filename, REPLACE(new.path, '/', ' '));
END;

CREATE TRIGGER assets_au AFTER UPDATE ON assets BEGIN
    UPDATE assets_fts
    SET filename = new.filename,
        path_segments = REPLACE(new.path, '/', ' ')
    WHERE rowid = new.id;
END;

CREATE TRIGGER assets_ad AFTER DELETE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
END;

-- Scanning progress tracking
CREATE TABLE scan_sessions (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL,
    total_files INTEGER,
    processed_files INTEGER DEFAULT 0,
    status TEXT DEFAULT 'running',  -- 'running', 'paused', 'complete', 'error'
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    error TEXT
);

-- SQLite optimizations
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA cache_size=-64000;         -- 64MB cache
PRAGMA temp_store=MEMORY;
```

**Size Optimization:**
- Store only small thumbnails (128px) as BLOBs (<10-20KB each)
- Normalize paths (single path column, not duplicated directories)
- No redundant data (embeddings, tags added in future phases)
- Estimated size: ~300MB for 10,000 assets (DB + thumbnails)

---

## Core Features (MVP)

### 1. Asset Scanning with Pause/Resume

**Rust Backend:**

```rust
use std::path::{Path, PathBuf};
use tokio::fs;
use walkdir::WalkDir;

#[derive(Clone, serde::Serialize)]
pub struct ScanProgress {
    pub session_id: i64,
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: String,
    pub status: String,
}

#[tauri::command]
pub async fn start_scan(
    state: State<'_, AppState>,
    window: Window,
    root_path: String,
) -> Result<i64, String> {
    // 1. Create scan session
    let session_id = create_scan_session(&state.db, &root_path).await?;

    // 2. Discover all supported files
    let files = discover_files(&root_path).await?;
    let total = files.len();

    update_session_total(&state.db, session_id, total).await?;

    // 3. Process in background with periodic progress updates
    tokio::spawn(async move {
        process_files(state, window, session_id, files).await
    });

    Ok(session_id)
}

#[tauri::command]
pub async fn pause_scan(
    state: State<'_, AppState>,
    session_id: i64,
) -> Result<(), String> {
    update_session_status(&state.db, session_id, "paused").await
}

#[tauri::command]
pub async fn resume_scan(
    state: State<'_, AppState>,
    window: Window,
    session_id: i64,
) -> Result<(), String> {
    // Get unprocessed files
    let files = get_unprocessed_files(&state.db, session_id).await?;

    update_session_status(&state.db, session_id, "running").await?;

    tokio::spawn(async move {
        process_files(state, window, session_id, files).await
    });

    Ok(())
}

async fn process_files(
    state: State<'_, AppState>,
    window: Window,
    session_id: i64,
    files: Vec<PathBuf>,
) -> Result<(), String> {
    const BATCH_SIZE: usize = 50;
    const PROGRESS_INTERVAL: usize = 10;

    for (batch_idx, batch) in files.chunks(BATCH_SIZE).enumerate() {
        // Check if paused
        if is_session_paused(&state.db, session_id).await? {
            break;
        }

        // Process batch in parallel
        let results = tokio::task::spawn_blocking({
            let batch = batch.to_vec();
            move || {
                use rayon::prelude::*;
                batch.par_iter()
                    .map(|path| process_single_asset(path))
                    .collect::<Vec<_>>()
            }
        }).await?;

        // Save to database
        save_batch_to_db(&state.db, results).await?;

        // Emit progress (throttled)
        if batch_idx % PROGRESS_INTERVAL == 0 {
            let progress = get_scan_progress(&state.db, session_id).await?;
            window.emit("scan-progress", progress)?;
        }
    }

    // Mark session complete
    update_session_status(&state.db, session_id, "complete").await?;
    window.emit("scan-complete", session_id)?;

    Ok(())
}
```

### 2. Asset Processing (Minimal)

**Image Processing:**

```rust
use image::ImageReader;
use fast_image_resize as fir;

fn process_single_asset(path: &Path) -> Result<AssetData, String> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    let modified = metadata.modified()?;

    let asset_type = detect_asset_type(path)?;

    match asset_type {
        AssetType::Image => process_image(path, file_size, modified),
        AssetType::Audio => process_audio(path, file_size, modified),
    }
}

fn process_image(
    path: &Path,
    file_size: u64,
    modified: SystemTime,
) -> Result<AssetData, String> {
    // 1. Load image
    let img = ImageReader::open(path)?.decode()?;
    let (width, height) = img.dimensions();

    // 2. Generate thumbnail (128px max dimension)
    let thumbnail = generate_thumbnail(&img, 128)?;

    Ok(AssetData {
        filename: path.file_name().unwrap().to_string_lossy().to_string(),
        path: path.to_string_lossy().to_string(),
        asset_type: "image".into(),
        format: get_format(path),
        file_size,
        width: Some(width),
        height: Some(height),
        thumbnail_data: thumbnail,
        modified_at: modified.duration_since(UNIX_EPOCH)?.as_secs() as i64,
    })
}

fn generate_thumbnail(img: &DynamicImage, max_size: u32) -> Result<Vec<u8>, String> {
    let (width, height) = img.dimensions();
    let scale = (max_size as f32 / width.max(height) as f32).min(1.0);

    let new_width = (width as f32 * scale) as u32;
    let new_height = (height as f32 * scale) as u32;

    // Use fast_image_resize for performance
    let src_image = fir::Image::from_vec_u8(
        width,
        height,
        img.to_rgba8().into_raw(),
        fir::PixelType::U8x4,
    )?;

    let mut dst_image = fir::Image::new(
        new_width,
        new_height,
        fir::PixelType::U8x4,
    );

    let mut resizer = fir::Resizer::new(fir::ResizeAlg::Convolution(
        fir::FilterType::Lanczos3
    ));

    resizer.resize(&src_image.view(), &mut dst_image.view_mut())?;

    // Encode as JPEG (smaller than PNG for thumbnails)
    let mut buffer = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 85);
    encoder.encode(
        dst_image.buffer(),
        new_width,
        new_height,
        image::ColorType::Rgba8,
    )?;

    Ok(buffer)
}
```

**Audio Processing:**

```rust
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

fn process_audio(
    path: &Path,
    file_size: u64,
    modified: SystemTime,
) -> Result<AssetData, String> {
    // Open audio file
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(ext.to_str().unwrap());
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;

    let format = probed.format;
    let track = format.default_track().unwrap();

    // Extract metadata
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.unwrap_or(0);
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(0) as i32;

    // Calculate duration
    let duration_ms = if let Some(n_frames) = codec_params.n_frames {
        (n_frames as f64 / sample_rate as f64 * 1000.0) as i64
    } else {
        0
    };

    Ok(AssetData {
        filename: path.file_name().unwrap().to_string_lossy().to_string(),
        path: path.to_string_lossy().to_string(),
        asset_type: "audio".into(),
        format: get_format(path),
        file_size,
        duration_ms: Some(duration_ms),
        sample_rate: Some(sample_rate as i32),
        channels: Some(channels),
        thumbnail_data: vec![], // No thumbnail for audio in MVP
        modified_at: modified.duration_since(UNIX_EPOCH)?.as_secs() as i64,
    })
}
```

### 3. Basic Search (FTS5)

**Rust Backend:**

```rust
#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub asset_type: Option<String>,  // 'image' or 'audio'
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    pub sort_by: SortBy,
    pub limit: u32,
    pub offset: u32,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Relevance,
    Name,
    DateModified,
    Size,
}

#[tauri::command]
pub async fn search_assets(
    state: State<'_, AppState>,
    query: SearchQuery,
) -> Result<Vec<Asset>, String> {
    let conn = state.db.lock().await;

    let mut sql = String::from("SELECT a.* FROM assets a");
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // Full-text search
    if let Some(text) = &query.text {
        if !text.is_empty() {
            sql.push_str(" INNER JOIN assets_fts fts ON a.id = fts.rowid");
            conditions.push("fts MATCH ?");
            params.push(Box::new(format!("{}*", text.trim())));
        }
    }

    // Asset type filter
    if let Some(asset_type) = &query.asset_type {
        conditions.push("a.asset_type = ?");
        params.push(Box::new(asset_type.clone()));
    }

    // Dimension filters
    if let Some(min_width) = query.min_width {
        conditions.push("a.width >= ?");
        params.push(Box::new(min_width));
    }
    if let Some(min_height) = query.min_height {
        conditions.push("a.height >= ?");
        params.push(Box::new(min_height));
    }

    // Only show complete assets
    conditions.push("a.processing_status = 'complete'");

    // Build WHERE clause
    if !conditions.is_empty() {
        sql.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    // Sorting
    sql.push_str(&match query.sort_by {
        SortBy::Relevance => " ORDER BY bm25(fts) ASC",
        SortBy::Name => " ORDER BY a.filename COLLATE NOCASE ASC",
        SortBy::DateModified => " ORDER BY a.modified_at DESC",
        SortBy::Size => " ORDER BY a.file_size DESC",
    });

    // Pagination
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit, query.offset));

    // Execute
    let mut stmt = conn.prepare(&sql)?;
    let results = stmt.query_map(params.as_slice(), |row| {
        Ok(Asset::from_row(row))
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

#[tauri::command]
pub async fn get_thumbnail(
    state: State<'_, AppState>,
    asset_id: i64,
) -> Result<Vec<u8>, String> {
    let conn = state.db.lock().await;

    let thumbnail: Vec<u8> = conn.query_row(
        "SELECT thumbnail_data FROM assets WHERE id = ?",
        params![asset_id],
        |row| row.get(0),
    )?;

    Ok(thumbnail)
}
```

### 4. Frontend Display

**Svelte Table/Grid Component:**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import VirtualList from 'svelte-virtual';
  import { searchState } from '$lib/state/search.svelte';

  interface Asset {
    id: number;
    filename: string;
    path: string;
    asset_type: string;
    file_size: number;
    width?: number;
    height?: number;
    duration_ms?: number;
  }

  let assets = $state<Asset[]>([]);
  let isLoading = $state(false);
  let viewMode = $state<'table' | 'grid'>('table');

  async function loadAssets() {
    isLoading = true;
    try {
      assets = await invoke<Asset[]>('search_assets', {
        query: {
          text: searchState.text,
          asset_type: searchState.assetType,
          sort_by: searchState.sortBy,
          limit: 100,
          offset: 0,
        }
      });
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    loadAssets();
  });
</script>

<div class="flex flex-col h-full">
  <!-- Controls -->
  <div class="flex items-center gap-4 p-4 border-b border-default">
    <input
      type="text"
      bind:value={searchState.text}
      placeholder="Search assets..."
      class="flex-1 px-3 py-2 border border-default rounded"
    />

    <select bind:value={viewMode} class="px-3 py-2 border border-default rounded">
      <option value="table">Table</option>
      <option value="grid">Grid</option>
    </select>
  </div>

  <!-- Results -->
  {#if viewMode === 'table'}
    <TableView {assets} {isLoading} />
  {:else}
    <GridView {assets} {isLoading} />
  {/if}
</div>
```

**Table View:**

```svelte
<script lang="ts">
  import type { Asset } from '$lib/types';

  interface Props {
    assets: Asset[];
    isLoading: boolean;
  }

  let { assets, isLoading }: Props = $props();

  function formatSize(bytes: number): string {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  }
</script>

<div class="flex-1 overflow-auto">
  <table class="w-full">
    <thead class="sticky top-0 bg-secondary border-b border-default">
      <tr>
        <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Preview</th>
        <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Name</th>
        <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Type</th>
        <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Dimensions</th>
        <th class="px-4 py-2 text-left text-sm font-medium text-secondary">Size</th>
      </tr>
    </thead>
    <tbody>
      {#if isLoading}
        <tr><td colspan="5" class="px-4 py-8 text-center text-secondary">Loading...</td></tr>
      {:else if assets.length === 0}
        <tr><td colspan="5" class="px-4 py-8 text-center text-secondary">No assets found</td></tr>
      {:else}
        {#each assets as asset}
          <tr class="border-b border-default hover:bg-secondary/50">
            <td class="px-4 py-2">
              <AssetThumbnail assetId={asset.id} type={asset.asset_type} />
            </td>
            <td class="px-4 py-2 text-sm text-primary">{asset.filename}</td>
            <td class="px-4 py-2 text-sm text-secondary">{asset.asset_type}</td>
            <td class="px-4 py-2 text-sm text-secondary">
              {#if asset.width && asset.height}
                {asset.width} × {asset.height}
              {:else if asset.duration_ms}
                {(asset.duration_ms / 1000).toFixed(1)}s
              {:else}
                —
              {/if}
            </td>
            <td class="px-4 py-2 text-sm text-secondary">{formatSize(asset.file_size)}</td>
          </tr>
        {/each}
      {/if}
    </tbody>
  </table>
</div>
```

---

## Implementation Timeline (4 Weeks)

### Week 1: Foundation
- Set up Tauri + Svelte project
- Initialize SQLite database with schema
- Create basic asset scanning (file discovery)
- Build simple file list display

### Week 2: Processing Pipeline
- Implement image processing (thumbnails, metadata)
- Implement audio processing (metadata)
- Add pause/resume functionality
- Build progress tracking UI

### Week 3: Search & Display
- Implement FTS5 search backend
- Create search UI with filters
- Build table view with virtual scrolling
- Build grid view with thumbnails

### Week 4: Polish
- Error handling and edge cases
- Performance optimization
- ZIP file support (stream reading)
- Basic settings (scan paths, preferences)

---

## Future Expansion Hooks

**Database Extensions (Post-MVP):**

```sql
-- Add columns for ML features later
ALTER TABLE assets ADD COLUMN embedding BLOB;
ALTER TABLE assets ADD COLUMN perceptual_hash TEXT;
ALTER TABLE assets ADD COLUMN auto_tags TEXT;  -- JSON array
ALTER TABLE assets ADD COLUMN processing_tier TEXT;  -- 'fast', 'quality', 'premium'

-- Add FTS columns for tags/descriptions
-- (requires recreating FTS table)
```

**Code Structure:**

```
src-tauri/src/
├── commands/
│   ├── scan.rs          # MVP: Basic scanning
│   ├── search.rs        # MVP: FTS5 search
│   └── ml.rs            # Future: ML processing
├── services/
│   ├── asset_processor.rs  # MVP: Basic metadata
│   └── ml_processor.rs     # Future: CLIP, PANNs, etc.
└── database/
    ├── schema.rs        # Core schema
    └── migrations/      # Future schema updates
```

---

## Performance Targets (MVP)

| Metric | Target | Method |
|--------|--------|--------|
| Scan rate | 50-100 files/sec | Parallel processing with Rayon |
| Per-asset time | 20-50ms | Fast thumbnail generation only |
| Search latency | <50ms | FTS5 + indexes |
| Table scroll | 60 FPS | Virtual scrolling |
| DB size | <30KB per asset | Thumbnails only, no embeddings |
| Total for 10K | ~300MB | Efficient JPEG thumbnails |

---

## Critical Success Factors

1. **Progress Feedback**: Real-time progress during scanning
2. **Pause/Resume**: Don't block user during long operations
3. **Fast Search**: Sub-50ms FTS5 queries with proper indexes
4. **Efficient Storage**: Small thumbnails, no redundant data
5. **Virtual Scrolling**: Handle 10,000+ items smoothly
6. **Error Recovery**: Handle corrupted files gracefully

---

## Dependencies

**Cargo.toml (Rust):**

```toml
[dependencies]
tauri = "2.0"
tokio = { version = "1.40", features = ["full"] }
rayon = "1.10"
rusqlite = { version = "0.32", features = ["bundled"] }
image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }
fast_image_resize = "5.0"
symphonia = "0.5"
walkdir = "2.5"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**package.json (Frontend):**

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "svelte": "^5.0.0",
    "@sveltejs/kit": "^2.0.0",
    "svelte-virtual": "^1.0.0"
  }
}
```

---

## Summary

This MVP focuses on **core asset management** with minimal overhead:

✅ **Fast scanning** with pause/resume
✅ **Basic search** with FTS5 (filename, path)
✅ **Table/grid display** with virtual scrolling
✅ **Small DB footprint** (~300MB for 10K assets)
✅ **Extensible architecture** for future ML features

**Total implementation time**: 4 weeks for working MVP.

**Post-MVP roadmap**:
- Phase 2: ML tagging (Fast tier - CLIP/PANNs)
- Phase 3: Advanced search (tags, similarity)
- Phase 4: Canvas visualization
- Phase 5: Duplicate detection
- Phase 6: Quality/Premium tiers
