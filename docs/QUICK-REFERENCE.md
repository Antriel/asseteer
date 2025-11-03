# Quick Reference Guide

Fast lookup for models, API commands, database operations, and common patterns.

---

## Quality Tier Comparison

| Aspect | Fast | Quality | Premium |
|--------|------|---------|---------|
| **Image Model** | CLIP ViT-B/32 | CLIP ViT-L/14 | LLaVA 1.6 7B |
| **Audio Model** | PANNs CNN14 | CLAP | Audio LLM |
| **Processing** | 51-106ms/51-106ms | 85-168ms/37-75ms | 235-835ms |
| **VRAM Peak** | 3.3-4.6GB | 3.9-4.7GB | 11.5-15.5GB |
| **Tag Depth** | Basic | Detailed | Rich descriptions |
| **Search Quality** | Good | Very Good | Excellent |
| **Cost** | Low | Medium | High |

---

## Model Specifications

### Image Models

```
CLIP ViT-B/32 (Fast)
├─ Size: 289MB
├─ Speed: 16ms
├─ VRAM: 500MB-800MB
└─ Output: Basic tags + style

CLIP ViT-L/14 (Quality)
├─ Size: 890MB
├─ Speed: 50-80ms
├─ VRAM: 1.5-2GB
└─ Output: Detailed tags + style confidence

SigLIP ViT-L/16 (Quality alt)
├─ Size: Similar to ViT-L/14
├─ Speed: 50-80ms
├─ VRAM: 1.5-2GB
├─ Training: WebLI (higher quality data)
└─ Output: Better text-image alignment

LLaVA-1.6 7B (Premium)
├─ Size: 5-7GB (INT8 quantized)
├─ Speed: 200-500ms
├─ VRAM: 5-7GB
├─ Params: 7 billion
└─ Output: Rich natural language descriptions
```

### Audio Models

```
PANNs CNN14 (Fast)
├─ Size: 80MB
├─ Speed: 5-10ms
├─ VRAM: 2-3GB
├─ Classes: AudioSet 527
└─ Output: Basic categories + confidence

CLAP (Quality)
├─ Size: 630MB
├─ Speed: 20-40ms
├─ VRAM: 1.2-1.5GB
├─ Type: Contrastive Language-Audio
└─ Output: Better text-audio alignment

BEATs (Quality alt)
├─ Size: Similar to CLAP
├─ Speed: 50-100ms
├─ VRAM: 1-2GB
├─ Type: Bidirectional Encoder
└─ Output: State-of-the-art understanding

Audio LLM (Premium)
├─ Options: Qwen2-Audio, SALMONN
├─ Speed: 300-800ms
├─ VRAM: 4-6GB (INT8)
└─ Output: Temporal breakdowns + descriptions
```

---

## Database Quick Lookup

### Asset Record Structure

```sql
-- Core metadata
id, name, path, zip_entry, asset_type, format, file_size

-- Image fields
width, height, aspect_ratio, is_spritesheet, grid_dimensions

-- Audio fields
duration_ms, duration_category, sample_rate, channels

-- ML data
processing_tier, auto_tags (JSON), manual_tags (JSON),
premium_description, style_primary, style_confidence,
sound_category, sound_subcategory, category_confidence

-- Vectors
embedding (BLOB), perceptual_hash

-- Layout
position_x, position_y, cluster_id

-- Thumbnails
thumbnail_small (BLOB), thumbnail_medium_path, thumbnail_large_path

-- Timestamps
created_at, modified_at, last_accessed, cache_version
```

### FTS5 Columns

```
name, path_segments, auto_tags, manual_tags,
style_desc, sound_desc, premium_desc
```

### Common Queries

```sql
-- Search with ranking
SELECT a.*, bm25(fts) as score FROM assets a
INNER JOIN assets_fts fts ON a.id = fts.rowid
WHERE fts MATCH ?
ORDER BY score ASC LIMIT 50;

-- Find similar
SELECT id, (dot_product / (norm_a * norm_b)) as similarity
FROM assets WHERE id != ? AND asset_type = ?
ORDER BY similarity DESC LIMIT 20;

-- Get duplicates
SELECT * FROM duplicates
WHERE similarity_score > ?
ORDER BY similarity_score DESC;

-- Faceted counts
SELECT style_primary, COUNT(*) FROM assets
WHERE asset_type = 'image'
GROUP BY style_primary;

-- Asset by tier
SELECT id, name, processing_tier FROM assets
WHERE processing_tier = ? ORDER BY modified_at DESC;
```

---

## API Command Quick Reference

### Import

```rust
invoke('import_assets', {
  paths: Vec<String>,
  qualityTier: 'fast' | 'quality' | 'premium'
}) → ImportJob
```

### Search

```rust
invoke('search_assets', {
  query: {
    text?: string,
    assetType?: 'image' | 'audio',
    styles?: string[],
    soundCategories?: string[],
    durationCategory?: string,
    tags?: string[],
    sortBy: 'relevance' | 'name' | 'date_modified' | 'date_created',
    limit: u32,
    offset: u32
  }
}) → AssetSearchResult[]
```

### Similarity

```rust
invoke('find_similar_assets', {
  assetId: i64,
  limit: u32
}) → (AssetId, Similarity)[]

invoke('hybrid_search', {
  textQuery: string,
  referenceAssetId?: i64,
  limit: u32
}) → AssetSearchResult[]
```

### Duplicates

```rust
invoke('find_duplicates', {
  similarityThreshold?: f32  // default 0.95
}) → Vec<Vec<AssetId>>

invoke('resolve_duplicate', {
  primaryAssetId: i64,
  duplicateIds: Vec<i64>,
  action: 'keep_primary' | 'delete_all_duplicates' | 'merge_tags'
}) → ()
```

### Tags

```rust
invoke('add_manual_tag', {
  assetId: i64,
  tag: string
}) → ()

invoke('remove_manual_tag', {
  assetId: i64,
  tag: string
}) → ()

invoke('get_asset_tags', {
  assetId: i64
}) → AssetTags
```

### Canvas

```rust
invoke('get_canvas_data', {
  assetType?: string
}) → CanvasData  // {assets, clusters}

invoke('get_thumbnail', {
  assetId: i64,
  lodLevel: 'small' | 'medium' | 'large'
}) → Uint8Array

invoke('get_asset_detail', {
  assetId: i64
}) → AssetDetail
```

### Premium

```rust
invoke('ask_about_asset', {
  assetId: i64,
  question: string
}) → string  // LLM answer

invoke('upgrade_assets_quality', {
  assetIds?: Vec<i64>,  // null = all
  newTier: 'fast' | 'quality' | 'premium'
}) → ReprocessingJob
```

### Settings

```rust
invoke('get_processing_config') → ProcessingConfig

invoke('set_processing_config', {
  config: ProcessingConfig
}) → ()

invoke('get_job_status', {
  jobId: string
}) → JobStatus

invoke('cancel_job', {
  jobId: string
}) → ()
```

---

## Frontend State Management Patterns

### Export State Directly

```typescript
// src/lib/state/myFeature.svelte.ts

// ✅ CORRECT - Export state directly
export const myState = $state<MyType>({ value: 0 });

// ✅ CORRECT - Export getter functions
export function getDerivedValue(): number {
  return myState.value * 2;
}

// ❌ WRONG - Cannot export $derived directly
// export const derived = $derived(myState.value * 2);
```

### Use State in Components

```svelte
<script lang="ts">
  import { myState, getDerivedValue } from '$lib/state/myFeature.svelte';

  // Direct access - automatic reactivity!
  const doubled = $derived(getDerivedValue());

  function handleChange() {
    myState.value++;  // State updates automatically
  }
</script>

<p>{myState.value}</p>
<p>{doubled}</p>
```

### Async Operations

```typescript
// src/lib/state/search.svelte.ts

export const results = $state<Asset[]>([]);
export const isLoading = $state(false);
export const error = $state<string | null>(null);

export async function performSearch(query: string) {
  isLoading = true;
  error = null;

  try {
    const data = await invoke('search_assets', { query });
    results.length = 0;
    results.push(...data);
  } catch (err) {
    error = String(err);
  } finally {
    isLoading = false;
  }
}
```

---

## Common Implementation Patterns

### I/O + CPU Work Separation

```rust
#[tauri::command]
async fn process_batch(paths: Vec<String>) -> Result<Vec<Data>, String> {
    // 1. I/O: Read files asynchronously
    let data = futures::future::join_all(
        paths.iter().map(|p| tokio::fs::read(p))
    ).await;

    // 2. CPU: Heavy work in spawn_blocking
    let processed = tokio::task::spawn_blocking({
        let data = data.into_iter().collect::<Result<Vec<_>, _>>()?;
        move || {
            use rayon::prelude::*;
            data.par_iter()
                .map(|bytes| expensive_processing(bytes))
                .collect::<Vec<_>>()
        }
    }).await?;

    Ok(processed)
}
```

### Error Handling with Toast

```typescript
try {
  await invoke('some_command', { ... });
  showToast('Operation succeeded', 'success');
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  showToast(`Failed: ${message}`, 'error');
}
```

### Confirmation Dialog

```typescript
async function handleDelete(id: number) {
  const confirmed = await showConfirm(
    `Delete asset ${id}? This cannot be undone.`,
    'Confirm Deletion',
    'Delete'
  );

  if (confirmed) {
    // Proceed with deletion
  }
}
```

### Debounced Search

```typescript
import { debounce } from 'lodash-es';

const debouncedSearch = debounce(async (query: string) => {
  const results = await invoke('search_assets', { query });
  searchResults.length = 0;
  searchResults.push(...results);
}, 300);

$effect(() => {
  debouncedSearch(searchQuery);
});
```

### Tailwind + Semantic Colors

```svelte
<!-- Use semantic color classes from app.css -->
<div class="flex items-center gap-2 px-4 py-2 bg-secondary border-b border-default">
  <span class="text-sm font-medium text-secondary">Label:</span>
  <span class="text-sm font-semibold text-primary">{value}</span>
</div>

<!-- CSS variables available: -->
<!-- --color-bg-primary, --color-bg-secondary -->
<!-- --color-text-primary, --color-text-secondary -->
<!-- --color-border, --color-accent -->
```

---

## File Paths & Locations

### Config Files
- Database: `~/.config/asseteer/asseteer.db`
- Models: `~/.cache/asseteer/models/`
- Thumbnails: `~/.cache/asseteer/thumbnails/`

### Code Structure
- Commands: `src-tauri/src/commands/*.rs`
- Database: `src-tauri/src/database/*.rs`
- ML: `src-tauri/src/ml/*.rs`
- State: `src/lib/state/*.svelte.ts`
- Components: `src/lib/components/**/*.svelte`

---

## Debugging & Profiling

### Enable Debug Logging

```rust
// In Rust
eprintln!("Debug info: {:?}", data);

// Use RUST_LOG environment variable
RUST_LOG=debug cargo run
```

### Profile Performance

```bash
# Backend CPU profiling
cargo flamegraph --bin asseteer

# Frontend DevTools
# Open in browser, use Chrome Profiler on canvas
```

### Database Inspection

```bash
# Open database
sqlite3 ~/.config/asseteer/asseteer.db

# Check schema
.schema assets

# Count assets
SELECT COUNT(*) FROM assets;

# Check FTS5 index
INSERT INTO assets_fts(assets_fts) VALUES('optimize');
```

---

## Troubleshooting Checklist

| Issue | Solution |
|-------|----------|
| Models not found | Run model downloader, check cache dir |
| Slow search | Run `OPTIMIZE` on FTS5, check indices |
| High VRAM usage | Switch to Fast tier, reduce batch size |
| Assets not appearing | Check database connection, verify import |
| Canvas frozen | Check LOD level, reduce visible items |
| Duplicates not detected | Ensure embeddings generated, check threshold |

---

## Resources & References

### Documentation
- SQLite FTS5: https://www.sqlite.org/fts5.html
- PixiJS: https://pixijs.download/release/docs/index.html
- Tauri: https://tauri.app/
- CLIP: https://github.com/openai/CLIP
- PANNs: https://github.com/qiuqiangkong/panns_transfer_to_sound_event_detection

### Key Crates
```toml
ort = "2.0"              # ONNX Runtime
ndarray = "0.15"         # Numerical computing
image = "0.25"           # Image processing
rusqlite = "0.32"        # SQLite bindings
tauri = "2.0"            # Desktop framework
tokio = "1.40"           # Async runtime
rayon = "1.10"           # Parallel iterators
```
