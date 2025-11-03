# API Commands Reference

All commands are Tauri invocations (async) defined in `src-tauri/src/commands/`.

## Asset Management

### import_assets
Import assets from filesystem or ZIP archives.

```rust
#[tauri::command]
pub async fn import_assets(
    state: State<'_, SharedState>,
    paths: Vec<PathBuf>,
    quality_tier: QualityTier,
) -> Result<ImportJob, String>;

pub struct ImportJob {
    pub job_id: String,
    pub asset_count: usize,
    pub estimated_minutes: f32,
}
```

**Frontend Usage**:
```typescript
const job = await invoke('import_assets', {
  paths: ['/path/to/file.png', '/path/to/archive.zip'],
  qualityTier: 'quality',
});
```

---

### get_thumbnail
Retrieve thumbnail at specified LOD level.

```rust
#[tauri::command]
async fn get_thumbnail(
    state: State<'_, SharedState>,
    asset_id: i64,
    lod_level: String,  // 'small' | 'medium' | 'large'
) -> Result<Vec<u8>, String>
```

**Response**: Raw image bytes (PNG/JPEG)

**Frontend Usage**:
```typescript
const thumbBytes = await invoke<Uint8Array>('get_thumbnail', {
  assetId: 123,
  lodLevel: 'medium',
});
```

---

### get_audio_preview
Retrieve audio file for playback.

```rust
#[tauri::command]
async fn get_audio_preview(
    state: State<'_, SharedState>,
    asset_id: i64,
) -> Result<Vec<u8>, String>
```

**Response**: Raw audio bytes (WAV/MP3)

---

## Search & Filtering

### search_assets
Full-text and faceted search with optional vector reranking.

```rust
#[tauri::command]
pub async fn search_assets(
    state: State<'_, SharedState>,
    query: SearchQuery,
) -> Result<Vec<AssetSearchResult>, String>;

pub struct SearchQuery {
    pub text: Option<String>,                        // FTS5 query
    pub asset_type: Option<String>,                  // 'image' | 'audio'
    pub styles: Option<Vec<String>>,                 // For images
    pub sound_categories: Option<Vec<String>>,       // For audio
    pub duration_category: Option<String>,           // For audio
    pub min_width: Option<u32>,                      // For images
    pub min_height: Option<u32>,                     // For images
    pub tags: Option<Vec<String>>,                   // Manual tags filter
    pub is_spritesheet: Option<bool>,                // For images
    pub sort_by: SortBy,                             // See enum below
    pub limit: u32,
    pub offset: u32,
}

pub enum SortBy {
    Relevance,
    Name,
    DateModified,
    DateCreated,
    Size,
    Duration,
}

pub struct AssetSearchResult {
    pub id: i64,
    pub name: String,
    pub asset_type: String,
    pub thumbnail_url: String,
    pub relevance_score: f32,
    pub tags: Vec<String>,
    // ... other fields
}
```

**Frontend Usage**:
```typescript
const results = await invoke<Asset[]>('search_assets', {
  query: {
    text: 'knight character',
    assetType: 'image',
    styles: ['pixel_art'],
    sortBy: 'relevance',
    limit: 50,
    offset: 0,
  }
});
```

---

### find_similar_assets
Find assets with similar embeddings (vector similarity).

```rust
#[tauri::command]
pub async fn find_similar_assets(
    state: State<'_, SharedState>,
    asset_id: i64,
    limit: u32,
) -> Result<Vec<SimilarAsset>, String>;

pub struct SimilarAsset {
    pub id: i64,
    pub name: String,
    pub asset_type: String,
    pub similarity: f32,  // 0.0-1.0, cosine similarity
}
```

**Frontend Usage**:
```typescript
const similar = await invoke<SimilarAsset[]>('find_similar_assets', {
  assetId: 123,
  limit: 10,
});
```

---

### hybrid_search
Combine text relevance with vector similarity reranking.

```rust
#[tauri::command]
pub async fn hybrid_search(
    state: State<'_, SharedState>,
    text_query: String,
    reference_asset_id: Option<i64>,  // If provided, rerank by similarity to this asset
    limit: u32,
) -> Result<Vec<AssetSearchResult>, String>
```

**Use Case**: "Show me search results like this one" - combines text match with visual/audio similarity.

---

### get_facets
Get available filter options and their counts.

```rust
#[tauri::command]
pub async fn get_facets(
    state: State<'_, SharedState>,
    asset_type: Option<String>,
) -> Result<Facets, String>;

pub struct Facets {
    pub styles: Vec<(String, u32)>,              // (style, count)
    pub sound_categories: Vec<(String, u32)>,    // (category, count)
    pub duration_categories: Vec<(String, u32)>, // (category, count)
    pub tags: Vec<(String, u32)>,                // (tag, count)
}
```

**Frontend Usage**:
```typescript
const facets = await invoke<Facets>('get_facets', {
  assetType: 'image',
});
```

---

## Premium Tier Features

### ask_about_asset
Ask a question about a specific asset (Premium tier only).

```rust
#[tauri::command]
pub async fn ask_about_asset(
    state: State<'_, SharedState>,
    asset_id: i64,
    question: String,
) -> Result<String, String>;
```

**Examples**:
- "What animation frames are included in this sprite sheet?"
- "What color scheme does this character use?"
- "Is this sound suitable for indoor or outdoor scenes?"

**Requirements**:
- Asset must be processed with Premium tier
- Asset must have `premium_description` populated

---

## Asset Quality & Duplicates

### upgrade_assets_quality
Reprocess assets to a higher quality tier.

```rust
#[tauri::command]
pub async fn upgrade_assets_quality(
    state: State<'_, SharedState>,
    asset_ids: Option<Vec<i64>>,  // None = all assets
    new_tier: QualityTier,
) -> Result<ReprocessingJob, String>;

pub struct ReprocessingJob {
    pub job_id: String,
    pub asset_count: usize,
    pub estimated_minutes: f32,
}
```

**Frontend Usage**:
```typescript
const job = await invoke('upgrade_assets_quality', {
  assetIds: [1, 2, 3],  // Upgrade specific assets
  newTier: 'quality',
});
```

---

### find_duplicates
Detect duplicate or very similar assets.

```rust
#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, SharedState>,
    similarity_threshold: f32,  // 0.0-1.0, default 0.95
) -> Result<Vec<DuplicateGroup>, String>;

pub struct DuplicateGroup {
    pub primary_asset_id: i64,
    pub duplicates: Vec<DuplicatePair>,
}

pub struct DuplicatePair {
    pub asset_id: i64,
    pub similarity: f32,
    pub detection_method: String,  // 'perceptual_hash', 'chromaprint', 'embedding'
}
```

---

### resolve_duplicate
Mark one asset as the canonical version, hide/delete duplicates.

```rust
#[tauri::command]
pub async fn resolve_duplicate(
    state: State<'_, SharedState>,
    primary_asset_id: i64,
    duplicate_ids: Vec<i64>,
    action: DuplicateAction,  // 'keep_primary' | 'delete_all_duplicates' | 'merge_tags'
) -> Result<(), String>
```

---

## Tag Management

### add_manual_tag
Add a user-defined tag to an asset.

```rust
#[tauri::command]
pub async fn add_manual_tag(
    state: State<'_, SharedState>,
    asset_id: i64,
    tag: String,
) -> Result<(), String>
```

---

### remove_manual_tag
Remove a user-defined tag from an asset.

```rust
#[tauri::command]
pub async fn remove_manual_tag(
    state: State<'_, SharedState>,
    asset_id: i64,
    tag: String,
) -> Result<(), String>
```

---

### get_asset_tags
Get all tags (auto-generated and manual) for an asset.

```rust
#[tauri::command]
pub async fn get_asset_tags(
    state: State<'_, SharedState>,
    asset_id: i64,
) -> Result<AssetTags, String>;

pub struct AssetTags {
    pub auto_tags: Vec<String>,
    pub manual_tags: Vec<String>,
    pub style: Option<String>,
    pub category: Option<String>,
}
```

---

## Canvas & Visualization

### get_canvas_data
Get layout coordinates and cluster information for all assets.

```rust
#[tauri::command]
pub async fn get_canvas_data(
    state: State<'_, SharedState>,
    asset_type: Option<String>,  // Filter by type
) -> Result<CanvasData, String>;

pub struct CanvasData {
    pub assets: Vec<AssetCanvasItem>,
    pub clusters: Vec<ClusterInfo>,
}

pub struct AssetCanvasItem {
    pub id: i64,
    pub name: String,
    pub position_x: f32,
    pub position_y: f32,
    pub cluster_id: i32,
    pub asset_type: String,
}

pub struct ClusterInfo {
    pub id: i32,
    pub centroid_x: f32,
    pub centroid_y: f32,
    pub item_count: u32,
    pub label: Option<String>,  // Auto-generated label
}
```

---

### get_asset_detail
Get complete information about a single asset.

```rust
#[tauri::command]
pub async fn get_asset_detail(
    state: State<'_, SharedState>,
    asset_id: i64,
) -> Result<AssetDetail, String>;

pub struct AssetDetail {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub asset_type: String,
    pub format: String,

    // Image-specific
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub is_spritesheet: Option<bool>,
    pub grid_dimensions: Option<String>,

    // Audio-specific
    pub duration_ms: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,

    // ML-derived
    pub auto_tags: Vec<String>,
    pub manual_tags: Vec<String>,
    pub style: Option<String>,
    pub style_confidence: Option<f32>,
    pub sound_category: Option<String>,
    pub premium_description: Option<String>,

    // Created/modified
    pub created_at: u64,
    pub modified_at: u64,
}
```

---

## Job Management

### get_job_status
Get status of long-running operation.

```rust
#[tauri::command]
pub async fn get_job_status(
    state: State<'_, SharedState>,
    job_id: String,
) -> Result<JobStatus, String>;

pub struct JobStatus {
    pub job_id: String,
    pub status: JobState,  // 'queued' | 'running' | 'completed' | 'failed'
    pub progress: ProgressInfo,
}

pub struct ProgressInfo {
    pub current: u32,
    pub total: u32,
    pub percent: f32,
    pub estimated_remaining_ms: Option<u64>,
}
```

---

### cancel_job
Cancel a running job.

```rust
#[tauri::command]
pub async fn cancel_job(
    state: State<'_, SharedState>,
    job_id: String,
) -> Result<(), String>
```

---

## Settings & Configuration

### get_processing_config
Get current quality tier and processing settings.

```rust
#[tauri::command]
pub async fn get_processing_config(
    state: State<'_, SharedState>,
) -> Result<ProcessingConfig, String>;

pub struct ProcessingConfig {
    pub quality_tier: QualityTier,
    pub image_model: ImageModelType,
    pub audio_model: AudioModelType,
    pub batch_size: usize,
    pub processing_mode: ProcessingMode,  // 'immediate' | 'background' | 'scheduled'
    pub scheduled_time: Option<String>,   // "22:00" for overnight
}
```

---

### set_processing_config
Update quality tier and processing settings.

```rust
#[tauri::command]
pub async fn set_processing_config(
    state: State<'_, SharedState>,
    config: ProcessingConfig,
) -> Result<(), String>
```

---

## Event Streaming

### Progress Events
```typescript
// Listen for job progress updates
appWindow.listen('job_progress', (event) => {
  const progress: ProgressInfo = event.payload;
  console.log(`${progress.percent}% complete`);
});
```

### Toast Events
```typescript
// Listen for toast notifications from backend
appWindow.listen('show_toast', (event) => {
  const toast: ToastMessage = event.payload;
  // Display toast in UI
});
```

---

## Error Handling

All commands return `Result<T, String>`. Error strings follow pattern:

```
"Operation failed: {reason}"
"Invalid input: {field}"
"Database error: {details}"
"ML inference failed: {details}"
```

**Frontend Error Handling**:
```typescript
try {
  const results = await invoke('search_assets', { ... });
} catch (error: any) {
  const message = error instanceof Error ? error.message : String(error);
  showToast(`Search failed: ${message}`, 'error');
}
```
