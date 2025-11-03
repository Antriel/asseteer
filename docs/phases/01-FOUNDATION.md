# Phase 1: Foundation

Initialize Tauri project, set up database, and build basic asset import/display.

**Deliverable**: Working app that imports assets and displays them in a paginated grid.

---

## Setup & Project Initialization

### Tauri Project Structure

```
asseteer/
├── src/                          # Frontend (SvelteKit)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── shared/
│   │   │   ├── layout/
│   │   │   └── assets/
│   │   ├── state/
│   │   │   ├── ui.svelte.ts
│   │   │   ├── assets.svelte.ts
│   │   │   └── search.svelte.ts
│   │   ├── types/
│   │   │   └── index.ts
│   │   └── utils/
│   ├── routes/
│   │   ├── +layout.svelte
│   │   └── +page.svelte
│   ├── app.html
│   └── app.css
│
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   └── import.rs
│   │   ├── database/
│   │   │   ├── mod.rs
│   │   │   └── init.rs
│   │   ├── models/
│   │   │   └── mod.rs
│   │   ├── utils/
│   │   │   └── mod.rs
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
│
└── docs/                         # Documentation
    ├── 01-CORE-ARCHITECTURE.md
    ├── 02-DATABASE-SCHEMA.md
    └── phases/
        ├── 01-FOUNDATION.md (this file)
        ├── 02-SEARCH-FILTERING.md
        └── 03-ADVANCED-FEATURES.md
```

---

## 1. Database Initialization

### 1.1 Create Database Module

**File**: `src-tauri/src/database/init.rs`

```rust
use rusqlite::{Connection, Result as SqlResult};
use std::path::Path;

pub fn init_database(path: &Path) -> SqlResult<Connection> {
    let conn = Connection::open(path)?;

    // Enable WAL mode for concurrency
    conn.execute_batch("
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-64000;
    ")?;

    // Create core tables
    create_assets_table(&conn)?;
    create_fts_table(&conn)?;
    create_tags_tables(&conn)?;
    create_duplicates_table(&conn)?;

    Ok(conn)
}

fn create_assets_table(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS assets (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            zip_entry TEXT,
            asset_type TEXT,
            format TEXT,
            file_size INTEGER,

            -- Image metadata
            width INTEGER,
            height INTEGER,
            aspect_ratio TEXT,
            is_spritesheet BOOLEAN DEFAULT 0,
            grid_dimensions TEXT,

            -- Audio metadata
            duration_ms INTEGER,
            duration_category TEXT,
            sample_rate INTEGER,
            channels INTEGER,

            -- Quality tier
            processing_tier TEXT DEFAULT 'fast',

            -- Tags & descriptions
            auto_tags TEXT,
            manual_tags TEXT,
            premium_description TEXT,

            -- Style/Category
            style_primary TEXT,
            style_confidence REAL,
            sound_category TEXT,
            sound_subcategory TEXT,
            category_confidence REAL,

            -- ML data
            embedding BLOB,
            perceptual_hash TEXT,
            position_x REAL,
            position_y REAL,
            cluster_id INTEGER,

            -- Thumbnails
            thumbnail_small BLOB,
            thumbnail_medium_path TEXT,
            thumbnail_large_path TEXT,

            -- Timestamps
            created_at INTEGER,
            modified_at INTEGER,
            last_accessed INTEGER,
            cache_version INTEGER DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
        CREATE INDEX IF NOT EXISTS idx_assets_tier ON assets(processing_tier);
        CREATE INDEX IF NOT EXISTS idx_assets_hash ON assets(perceptual_hash);
        CREATE INDEX IF NOT EXISTS idx_assets_modified ON assets(modified_at);
    ")
}

fn create_fts_table(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
        CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
            name,
            path_segments,
            auto_tags,
            manual_tags,
            style_desc,
            sound_desc,
            premium_desc,
            content=assets,
            content_rowid=id,
            tokenize='porter unicode61 remove_diacritics 1'
        );

        CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
            INSERT INTO assets_fts(rowid, name, path_segments, auto_tags, manual_tags, style_desc, sound_desc, premium_desc)
            VALUES (new.id, new.name, REPLACE(new.path, '/', ' '), new.auto_tags, new.manual_tags, new.style_primary, new.sound_category, new.premium_description);
        END;

        CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
            UPDATE assets_fts
            SET name = new.name, path_segments = REPLACE(new.path, '/', ' '), auto_tags = new.auto_tags, manual_tags = new.manual_tags, style_desc = new.style_primary, sound_desc = new.sound_category, premium_desc = new.premium_description
            WHERE rowid = new.id;
        END;

        CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
            DELETE FROM assets_fts WHERE rowid = old.id;
        END;
    ")
}

fn create_tags_tables(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL
        );

        CREATE TABLE IF NOT EXISTS asset_tags (
            asset_id INTEGER REFERENCES assets(id),
            tag_id INTEGER REFERENCES tags(id),
            PRIMARY KEY (asset_id, tag_id)
        );

        CREATE INDEX IF NOT EXISTS idx_asset_tags_asset ON asset_tags(asset_id);
    ")
}

fn create_duplicates_table(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS duplicates (
            id INTEGER PRIMARY KEY,
            asset_id INTEGER REFERENCES assets(id),
            duplicate_of INTEGER REFERENCES assets(id),
            similarity_score REAL,
            method TEXT,
            UNIQUE(asset_id, duplicate_of)
        );

        CREATE INDEX IF NOT EXISTS idx_duplicates_asset ON duplicates(asset_id);
    ")
}
```

**File**: `src-tauri/src/database/mod.rs`

```rust
pub mod init;

pub use init::init_database;
```

---

## 2. Data Models

**File**: `src-tauri/src/models/mod.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub asset_type: String, // 'image' or 'audio'
    pub format: String,
    pub file_size: i64,
    pub processing_tier: String,
    pub auto_tags: Option<String>, // JSON array
    pub manual_tags: Option<String>, // JSON array
    pub thumbnail_small: Option<Vec<u8>>,
    pub created_at: i64,
    pub modified_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportJob {
    pub job_id: String,
    pub asset_count: usize,
    pub estimated_minutes: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityTier {
    Fast,
    Quality,
    Premium,
}

impl ToString for QualityTier {
    fn to_string(&self) -> String {
        match self {
            QualityTier::Fast => "fast".into(),
            QualityTier::Quality => "quality".into(),
            QualityTier::Premium => "premium".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub quality_tier: String,
    pub batch_size: usize,
}
```

---

## 3. Import Command

**File**: `src-tauri/src/commands/import.rs`

```rust
use crate::models::{Asset, ImportJob, QualityTier};
use std::path::{Path, PathBuf};
use tauri::State;
use tokio::fs;
use rusqlite::Connection;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

#[tauri::command]
pub async fn import_assets(
    state: State<'_, crate::AppState>,
    paths: Vec<String>,
    quality_tier: String,
) -> Result<ImportJob, String> {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let job_id = Uuid::new_v4().to_string();
    let asset_count = count_assets(&paths).await?;

    // Spawn background task
    let state_clone = state.inner().clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        if let Err(e) = process_import(&state_clone, paths, quality_tier, job_id_clone).await {
            eprintln!("Import failed: {}", e);
        }
    });

    Ok(ImportJob {
        job_id,
        asset_count,
        estimated_minutes: 0.0, // Removed timeline estimates
    })
}

async fn count_assets(paths: &[PathBuf]) -> Result<usize, String> {
    let mut count = 0;

    for path in paths {
        if path.is_file() {
            let ext = path.extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if is_supported_format(&ext) {
                count += 1;
            } else if ext == "zip" {
                count += count_zip_entries(path).await?;
            }
        } else if path.is_dir() {
            count += scan_directory(path).await?;
        }
    }

    Ok(count)
}

async fn scan_directory(dir: &Path) -> Result<usize, String> {
    let mut count = 0;
    let mut entries = fs::read_dir(dir)
        .await
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    while let Some(entry) = entries.next_entry()
        .await
        .map_err(|e| format!("Failed to read entry: {}", e))?
    {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if is_supported_format(&ext) {
                count += 1;
            } else if ext == "zip" {
                count += count_zip_entries(&path).await?;
            }
        }
    }

    Ok(count)
}

fn is_supported_format(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "webp" | "avif" | "gif"
            | "mp3" | "wav" | "ogg" | "flac" | "m4a"
    )
}

async fn count_zip_entries(zip_path: &Path) -> Result<usize, String> {
    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open ZIP: {}", e))?;

    let archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read ZIP: {}", e))?;

    Ok(archive.len())
}

async fn process_import(
    state: &crate::AppState,
    paths: Vec<PathBuf>,
    quality_tier: String,
    job_id: String,
) -> Result<(), String> {
    let conn = state.db.lock().unwrap();

    for path in paths {
        if path.is_file() {
            import_file(&conn, &path, &quality_tier)?;
        } else if path.is_dir() {
            import_directory(&conn, &path, &quality_tier).await?;
        }
    }

    Ok(())
}

fn import_file(
    conn: &Connection,
    path: &Path,
    quality_tier: &str,
) -> Result<(), String> {
    let ext = path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !is_supported_format(&ext) {
        return Ok(()); // Skip unsupported formats
    }

    let asset_type = if is_image_format(&ext) { "image" } else { "audio" };
    let name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let path_str = path.to_string_lossy().to_string();
    let file_size = std::fs::metadata(path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO assets (name, path, asset_type, format, file_size, processing_tier, created_at, modified_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            name, path_str, asset_type, ext, file_size, quality_tier, now, now
        ],
    )
    .map_err(|e| format!("Failed to insert asset: {}", e))?;

    Ok(())
}

async fn import_directory(
    conn: &Connection,
    dir: &Path,
    quality_tier: &str,
) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .await
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    while let Some(entry) = entries.next_entry()
        .await
        .map_err(|e| format!("Failed to read entry: {}", e))?
    {
        let path = entry.path();
        if path.is_file() {
            import_file(conn, &path, quality_tier)?;
        }
    }

    Ok(())
}

fn is_image_format(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "avif" | "gif")
}
```

**File**: `src-tauri/src/commands/mod.rs`

```rust
pub mod import;

pub use import::import_assets;
```

---

## 4. Application State

**File**: `src-tauri/src/lib.rs`

```rust
pub mod commands;
pub mod database;
pub mod models;

use rusqlite::Connection;
use std::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub db: std::sync::Arc<Mutex<Connection>>,
}
```

**File**: `src-tauri/src/main.rs`

```rust
use asseteer::AppState;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

fn main() {
    let db_path = get_app_data_dir().join("asseteer.db");
    let conn = asseteer::database::init_database(&db_path)
        .expect("Failed to initialize database");

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            asseteer::commands::import_assets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn get_app_data_dir() -> PathBuf {
    let data_dir = tauri::api::path::data_dir().expect("Failed to get data dir");
    let app_dir = data_dir.join("asseteer");
    std::fs::create_dir_all(&app_dir).ok();
    app_dir
}
```

---

## 5. Frontend: Asset Grid Display

**File**: `src/lib/types/index.ts`

```typescript
export interface Asset {
  id: number;
  name: string;
  path: string;
  assetType: 'image' | 'audio';
  format: string;
  fileSize: number;
  processingTier: string;
  createdAt: number;
  modifiedAt: number;
  thumbnailSmall?: Uint8Array;
  autoTags?: string[];
  manualTags?: string[];
}

export interface ImportJob {
  jobId: string;
  assetCount: number;
  estimatedMinutes: number;
}
```

**File**: `src/lib/state/assets.svelte.ts`

```typescript
import type { Asset } from '$lib/types';

export const assets = $state<Asset[]>([]);
export const isLoading = $state(false);
export const pageSize = $state(50);
export const currentPage = $state(0);

export const paginatedAssets = $derived(
  assets.slice(currentPage * pageSize, (currentPage + 1) * pageSize)
);

export const totalPages = $derived(Math.ceil(assets.length / pageSize));

export function setCurrentPage(page: number) {
  currentPage = Math.max(0, Math.min(page, totalPages - 1));
}
```

**File**: `src/lib/components/shared/AssetCard.svelte`

```svelte
<script lang="ts">
  import type { Asset } from '$lib/types';

  interface Props {
    asset: Asset;
  }

  let { asset }: Props = $props();

  function getThumbnailUrl(): string {
    if (asset.thumbnailSmall) {
      const blob = new Blob([asset.thumbnailSmall], { type: 'image/png' });
      return URL.createObjectURL(blob);
    }
    return '';
  }

  const thumbUrl = $derived(getThumbnailUrl());
</script>

<div class="flex flex-col gap-2 p-4 rounded border border-default hover:bg-secondary">
  {#if thumbUrl}
    <img
      src={thumbUrl}
      alt={asset.name}
      class="w-full h-32 object-cover rounded"
    />
  {:else}
    <div class="w-full h-32 bg-secondary rounded flex items-center justify-center">
      <span class="text-sm text-secondary">{asset.assetType.toUpperCase()}</span>
    </div>
  {/if}

  <h3 class="text-sm font-semibold text-primary truncate">{asset.name}</h3>

  {#if asset.autoTags && asset.autoTags.length > 0}
    <div class="flex flex-wrap gap-1">
      {#each asset.autoTags.slice(0, 3) as tag}
        <span class="text-xs px-2 py-1 bg-accent/10 text-accent rounded">
          {tag}
        </span>
      {/each}
    </div>
  {/if}
</div>
```

**File**: `src/routes/+page.svelte`

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import { open } from '@tauri-apps/plugin-dialog';
  import { assets, paginatedAssets, currentPage, totalPages, setCurrentPage } from '$lib/state/assets.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import AssetCard from '$lib/components/shared/AssetCard.svelte';

  let qualityTier = $state('fast');
  let isImporting = $state(false);

  async function handleImport() {
    try {
      const selected = await open({
        multiple: true,
        directory: false,
      });

      if (!selected) return;

      isImporting = true;
      const paths = Array.isArray(selected) ? selected : [selected];

      const job = await invoke('import_assets', {
        paths,
        qualityTier,
      });

      showToast(`Importing ${job.assetCount} assets...`, 'info');
      // TODO: Poll for job completion
    } catch (error) {
      showToast(`Import failed: ${error}`, 'error');
    } finally {
      isImporting = false;
    }
  }
</script>

<div class="flex flex-col gap-6 p-6">
  <!-- Header -->
  <div class="flex justify-between items-center">
    <h1 class="text-3xl font-bold text-primary">Asset Manager</h1>
    <div class="flex gap-4">
      <select
        bind:value={qualityTier}
        disabled={isImporting}
        class="px-4 py-2 border border-default rounded"
      >
        <option value="fast">Fast</option>
        <option value="quality">Quality</option>
        <option value="premium">Premium</option>
      </select>
      <button
        onclick={handleImport}
        disabled={isImporting}
        class="px-6 py-2 bg-accent text-white rounded disabled:opacity-50"
      >
        {isImporting ? 'Importing...' : 'Import Assets'}
      </button>
    </div>
  </div>

  <!-- Asset Grid -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
    {#each paginatedAssets as asset (asset.id)}
      <AssetCard {asset} />
    {/each}
  </div>

  {#if paginatedAssets.length === 0}
    <div class="text-center py-12 text-secondary">
      <p>No assets imported yet. Click "Import Assets" to get started.</p>
    </div>
  {/if}

  <!-- Pagination -->
  {#if totalPages > 1}
    <div class="flex justify-center gap-2">
      <button
        onclick={() => setCurrentPage(currentPage - 1)}
        disabled={currentPage === 0}
        class="px-4 py-2 border border-default rounded disabled:opacity-50"
      >
        Previous
      </button>
      <span class="px-4 py-2">
        Page {currentPage + 1} of {totalPages}
      </span>
      <button
        onclick={() => setCurrentPage(currentPage + 1)}
        disabled={currentPage === totalPages - 1}
        class="px-4 py-2 border border-default rounded disabled:opacity-50"
      >
        Next
      </button>
    </div>
  {/if}
</div>
```

---

## 6. Cargo Dependencies

**File**: `src-tauri/Cargo.toml`

Add to `[dependencies]`:

```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tauri = { version = "2.0", features = ["dialog-open", "shell-open"] }
rusqlite = { version = "0.32", features = ["bundled", "chrono"] }
tokio = { version = "1.40", features = ["full"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
zip = { version = "2.2", default-features = false, features = ["deflate"] }
```

---

## 7. Testing the Foundation

### Manual Verification Checklist

- [ ] Tauri app launches without errors
- [ ] Database file created in AppData
- [ ] Import button opens file picker
- [ ] Assets appear in grid after import
- [ ] Pagination works (5+ pages)
- [ ] Quality tier selector shows all options
- [ ] Asset details visible (name, type, tags)
- [ ] App responsiveness on resize

### Database Verification

```bash
# Check if database initialized
sqlite3 ~/.config/asseteer/asseteer.db ".tables"

# Verify schema
sqlite3 ~/.config/asseteer/asseteer.db ".schema assets"

# Check imported assets
sqlite3 ~/.config/asseteer/asseteer.db "SELECT COUNT(*) FROM assets;"
```

---

## Next Steps

Once Phase 1 is complete and verified:
1. Move to Phase 2: Search & Filtering
2. Implement image preprocessing pipeline
3. Add audio decoding and thumbnail generation
