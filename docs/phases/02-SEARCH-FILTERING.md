# Phase 2: Search & Filtering

Add auto-tagging, full-text search, and intelligent filtering.

**Deliverable**: Complete search interface with FTS5, CLIP/PANNs auto-tagging, sprite sheet detection, and similarity-based layout.

---

## Overview

This phase adds ML-based asset understanding and search capability:
1. **Image Auto-Tagging**: CLIP ViT-B/32 or ViT-L/14 (configurable)
2. **Audio Auto-Tagging**: PANNs CNN14 or CLAP (configurable)
3. **Traditional CV**: Sprite sheet detection via FFT
4. **Full-Text Search**: SQLite FTS5 with complex queries
5. **Vector Similarity**: Find similar assets via embeddings
6. **Layout**: UMAP + HDBSCAN clustering for infinite canvas

---

## 1. ML Model Integration

### 1.1 Model Manager

**File**: `src-tauri/src/ml/mod.rs`

```rust
pub mod models;
pub mod image;
pub mod audio;

use std::path::PathBuf;

pub struct ModelManager {
    cache_dir: PathBuf,
}

impl ModelManager {
    pub fn new(cache_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&cache_dir).ok();
        Self { cache_dir }
    }

    pub fn get_model_path(&self, model_name: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.onnx", model_name))
    }

    pub fn model_exists(&self, model_name: &str) -> bool {
        self.get_model_path(model_name).exists()
    }
}
```

**File**: `src-tauri/src/ml/models.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub tier: String,                  // 'fast', 'quality', 'premium'
    pub asset_type: String,            // 'image' or 'audio'
    pub size_mb: u64,
    pub download_url: String,
    pub sha256: String,
}

pub const IMAGE_MODELS: &[(&str, &str, u64)] = &[
    ("clip-vit-b32", "fast", 289),
    ("clip-vit-l14", "quality", 890),
    ("llava-1.6-7b", "premium", 5000),
];

pub const AUDIO_MODELS: &[(&str, &str, u64)] = &[
    ("panns-cnn14", "fast", 80),
    ("clap-audio", "quality", 630),
    ("audio-llm-qwen", "premium", 4500),
];
```

---

## 2. Image Processing & CLIP Integration

### 2.1 CLIP Image Tagger

**File**: `src-tauri/src/ml/image.rs`

```rust
use ort::{Session, Value};
use ndarray::{Array, IxDyn};
use image::DynamicImage;
use std::path::Path;

pub struct CLIPImageTagger {
    vision_session: Session,
    text_session: Session,
}

#[derive(Debug, Clone)]
pub struct ImageTags {
    pub style: Option<String>,
    pub style_confidence: f32,
    pub tags: Vec<String>,
    pub is_spritesheet: bool,
    pub grid_dimensions: Option<String>,
}

impl CLIPImageTagger {
    pub fn new(model_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let vision_session = Session::builder()?
            .with_model_from_file(model_path.join("clip_vision.onnx"))?;

        let text_session = Session::builder()?
            .with_model_from_file(model_path.join("clip_text.onnx"))?;

        Ok(Self {
            vision_session,
            text_session,
        })
    }

    pub async fn generate_tags(&self, image: &DynamicImage) -> Result<ImageTags, String> {
        // Resize to 224x224
        let resized = image.resize_exact(224, 224, image::imageops::FilterType::Lanczos3);
        let tensor = self.image_to_tensor(&resized)?;

        // Get image embedding
        let image_input = Value::from_array_like(
            tensor.as_array_view(),
        ).map_err(|e| format!("Tensor creation failed: {}", e))?;

        let image_outputs = self.vision_session
            .run(vec![image_input])
            .map_err(|e| format!("Vision inference failed: {}", e))?;

        let image_embedding = image_outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Embedding extraction failed: {}", e))?;

        // Classify across tag categories
        let style_labels = vec![
            "pixel art sprite",
            "vector illustration",
            "3d rendered object",
            "hand drawn artwork",
            "photographic image",
        ];

        let content_labels = vec![
            "character sprite",
            "background landscape",
            "ui element",
            "tile texture",
            "icon symbol",
        ];

        let mut style = None;
        let mut style_confidence = 0.0;
        let mut tags = Vec::new();

        // Get style
        for (idx, label) in style_labels.iter().enumerate() {
            let score = self.calculate_similarity(
                image_embedding.as_slice().unwrap_or(&[]),
                label,
            ).await?;

            if score > 0.15 && score > style_confidence {
                style = Some(label.to_string());
                style_confidence = score;
            }
        }

        // Get content tags
        for (idx, label) in content_labels.iter().enumerate() {
            let score = self.calculate_similarity(
                image_embedding.as_slice().unwrap_or(&[]),
                label,
            ).await?;

            if score > 0.20 {
                tags.push(label.to_string());
            }
        }

        // Detect sprite sheets
        let (is_spritesheet, grid_dims) = detect_spritesheet(&resized)?;

        Ok(ImageTags {
            style,
            style_confidence,
            tags,
            is_spritesheet,
            grid_dimensions: grid_dims,
        })
    }

    async fn calculate_similarity(&self, image_emb: &[f32], text: &str) -> Result<f32, String> {
        // Tokenize text (placeholder - use actual tokenizer)
        let text_tensor = self.tokenize_text(text)?;

        let text_input = Value::from_array_like(
            &text_tensor,
        ).map_err(|e| format!("Text tensor failed: {}", e))?;

        let text_outputs = self.text_session
            .run(vec![text_input])
            .map_err(|e| format!("Text inference failed: {}", e))?;

        let text_emb = text_outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Text embedding failed: {}", e))?;

        let text_slice = text_emb.as_slice().unwrap_or(&[]);
        let similarity = cosine_similarity(image_emb, text_slice);

        Ok(similarity)
    }

    fn image_to_tensor(&self, img: &DynamicImage) -> Result<Array<f32, IxDyn>, String> {
        // Normalize to [-1, 1] and create tensor
        let rgb = img.to_rgb8();
        let mut data = Vec::new();

        for pixel in rgb.pixels() {
            data.push((pixel[0] as f32 / 127.5) - 1.0); // R
            data.push((pixel[1] as f32 / 127.5) - 1.0); // G
            data.push((pixel[2] as f32 / 127.5) - 1.0); // B
        }

        Ok(Array::from_shape_vec(
            IxDyn(&[1, 3, 224, 224]),
            data,
        ).map_err(|e| format!("Tensor shape error: {}", e))?)
    }

    fn tokenize_text(&self, text: &str) -> Result<Array<i64, IxDyn>, String> {
        // Placeholder: Use actual BPE tokenizer (e.g., tiktoken)
        let tokens = vec![0i64; 77]; // CLIP uses max 77 tokens
        Ok(Array::from_shape_vec(
            IxDyn(&[1, 77]),
            tokens,
        ).map_err(|e| format!("Tokenizer error: {}", e))?)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

// Sprite sheet detection via FFT
pub fn detect_spritesheet(img: &DynamicImage) -> Result<(bool, Option<String>), String> {
    let gray = img.to_luma8();
    let (width, height) = img.dimensions();

    if width < 64 || height < 64 {
        return Ok((false, None));
    }

    // Sum pixels along horizontal axis
    let mut h_signal = vec![0.0; width as usize];
    for (x, y, pixel) in gray.enumerate_pixels() {
        h_signal[x as usize] += pixel[0] as f32;
    }

    // Detect periodicity (FFT would go here)
    // For now, use simple heuristic: look for zero-crossings
    let periods = detect_periodicity(&h_signal);

    if !periods.is_empty() {
        let grid_x = periods[0];
        let cells_x = width / grid_x;

        // Repeat for vertical
        let mut v_signal = vec![0.0; height as usize];
        for (x, y, pixel) in gray.enumerate_pixels() {
            v_signal[y as usize] += pixel[0] as f32;
        }

        let periods_v = detect_periodicity(&v_signal);
        if !periods_v.is_empty() {
            let grid_y = periods_v[0];
            let cells_y = height / grid_y;

            if cells_x >= 2 && cells_y >= 2 && cells_x <= 32 && cells_y <= 32 {
                let grid_str = format!("{}x{}", cells_x, cells_y);
                return Ok((true, Some(grid_str)));
            }
        }
    }

    Ok((false, None))
}

fn detect_periodicity(signal: &[f32]) -> Vec<u32> {
    // Simple periodicity detection (replace with FFT for production)
    let mut periods = Vec::new();

    // Look for repeated patterns of zero crossings
    for period in 8..=128 {
        let mut confidence = 0.0;

        for i in 0..signal.len() {
            if i + period < signal.len() {
                let diff = (signal[i] - signal[i + period]).abs();
                if diff < 10.0 {
                    confidence += 1.0;
                }
            }
        }

        if confidence / (signal.len() as f32) > 0.8 {
            periods.push(period as u32);
        }
    }

    // Return most common period
    if !periods.is_empty() {
        periods.sort();
        vec![periods[periods.len() / 2]]
    } else {
        vec![]
    }
}
```

---

## 3. Audio Processing & PANNs Integration

### 3.1 Audio Tagger

**File**: `src-tauri/src/ml/audio.rs`

```rust
use ort::{Session, Value};
use ndarray::Array2;
use std::path::Path;

pub struct AudioTagger {
    panns_session: Session,
}

#[derive(Debug, Clone)]
pub struct AudioTags {
    pub category: String,
    pub tags: Vec<(String, f32)>, // tag, confidence
    pub duration_category: String,
}

impl AudioTagger {
    pub fn new(model_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let panns_session = Session::builder()?
            .with_model_from_file(model_path.join("panns_cnn14.onnx"))?;

        Ok(Self { panns_session })
    }

    pub async fn generate_tags(
        &self,
        audio_path: &Path,
        duration_ms: u32,
    ) -> Result<AudioTags, String> {
        // Load and decode audio
        let audio_data = load_audio(audio_path)?;

        // Create mel spectrogram
        let mel_spec = create_mel_spectrogram(&audio_data, 32000)?;

        // Run PANNs inference
        let input = Value::from_array_like(&mel_spec)
            .map_err(|e| format!("Mel spec tensor failed: {}", e))?;

        let outputs = self.panns_session
            .run(vec![input])
            .map_err(|e| format!("PANNs inference failed: {}", e))?;

        let logits = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Logits extraction failed: {}", e))?;

        // Apply sigmoid
        let probabilities: Vec<f32> = logits
            .iter()
            .map(|&x| 1.0 / (1.0 + (-x).exp()))
            .collect();

        // Get top tags
        let mut tags: Vec<(String, f32)> = vec![];
        let audioset_labels = get_audioset_labels();

        for (idx, &prob) in probabilities.iter().enumerate() {
            if prob > 0.3 && idx < audioset_labels.len() {
                tags.push((audioset_labels[idx].to_string(), prob));
            }
        }

        tags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        tags.truncate(10);

        let category = categorize_audio(&tags);
        let duration_category = categorize_duration(duration_ms);

        Ok(AudioTags {
            category,
            tags,
            duration_category,
        })
    }
}

fn load_audio(path: &Path) -> Result<Vec<f32>, String> {
    // Use symphonia for decoding
    // Placeholder implementation
    Ok(vec![0.0; 32000]) // 1 second at 32kHz
}

fn create_mel_spectrogram(audio: &[f32], sample_rate: u32) -> Result<Array2<f32>, String> {
    // Mel spectrogram computation
    // Returns (128, 128) for PANNs
    Ok(Array2::zeros((1, 128, 128)))
}

fn get_audioset_labels() -> Vec<&'static str> {
    vec![
        "speech", "music", "explosion", "impact", "footstep",
        "wind", "rain", "water", "door", "car",
        // ... 517 more AudioSet classes
    ]
}

fn categorize_audio(tags: &[(String, f32)]) -> String {
    for (label, _) in tags {
        let lower = label.to_lowercase();

        if lower.contains("speech") || lower.contains("voice") {
            return "voice".to_string();
        } else if lower.contains("music") || lower.contains("instrument") {
            return "music".to_string();
        } else if lower.contains("explosion") || lower.contains("impact") || lower.contains("bang") {
            return "impact".to_string();
        } else if lower.contains("ambient") || lower.contains("nature") || lower.contains("wind") {
            return "ambient".to_string();
        }
    }

    "effect".to_string()
}

fn categorize_duration(duration_ms: u32) -> String {
    match duration_ms {
        0..=500 => "very_short".to_string(),
        501..=2000 => "short".to_string(),
        2001..=5000 => "medium".to_string(),
        _ => "long".to_string(),
    }
}
```

---

## 4. Search Command

**File**: `src-tauri/src/commands/search.rs`

```rust
use crate::models::QualityTier;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub asset_type: Option<String>,
    pub styles: Option<Vec<String>>,
    pub sound_categories: Option<Vec<String>>,
    pub duration_category: Option<String>,
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    pub tags: Option<Vec<String>>,
    pub is_spritesheet: Option<bool>,
    pub sort_by: String,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetSearchResult {
    pub id: i64,
    pub name: String,
    pub asset_type: String,
    pub relevance_score: f32,
    pub auto_tags: Vec<String>,
    pub style: Option<String>,
}

#[tauri::command]
pub async fn search_assets(
    state: tauri::State<'_, crate::AppState>,
    query: SearchQuery,
) -> Result<Vec<AssetSearchResult>, String> {
    let conn = state.db.lock().unwrap();

    let mut sql = String::from(
        "SELECT DISTINCT a.id, a.name, a.asset_type, a.auto_tags, a.style_primary, bm25(fts) as relevance
         FROM assets a"
    );

    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // Full-text search
    if let Some(text) = &query.text {
        if !text.is_empty() {
            sql.push_str(" INNER JOIN assets_fts fts ON a.id = fts.rowid");
            conditions.push("fts MATCH ?");
            let fts_query = prepare_fts_query(text);
            params.push(Box::new(fts_query));
        }
    }

    // Asset type filter
    if let Some(asset_type) = &query.asset_type {
        conditions.push("a.asset_type = ?");
        params.push(Box::new(asset_type.clone()));
    }

    // Style filter
    if let Some(styles) = &query.styles {
        if !styles.is_empty() {
            let placeholders = vec!["?"; styles.len()].join(",");
            conditions.push(&format!("a.style_primary IN ({})", placeholders));
            for style in styles {
                params.push(Box::new(style.clone()));
            }
        }
    }

    // Add WHERE clause
    if !conditions.is_empty() {
        sql.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    // Sorting
    match query.sort_by.as_str() {
        "relevance" => sql.push_str(" ORDER BY relevance ASC"),
        "name" => sql.push_str(" ORDER BY a.name COLLATE NOCASE ASC"),
        "date_modified" => sql.push_str(" ORDER BY a.modified_at DESC"),
        _ => sql.push_str(" ORDER BY a.created_at DESC"),
    }

    // Pagination
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit, query.offset));

    let mut stmt = conn.prepare(&sql)
        .map_err(|e| format!("Query preparation failed: {}", e))?;

    let results = stmt.query_map(params.as_slice(), |row| {
        let auto_tags_json: String = row.get(3).unwrap_or_default();
        let auto_tags: Vec<String> = serde_json::from_str(&auto_tags_json).unwrap_or_default();

        Ok(AssetSearchResult {
            id: row.get(0)?,
            name: row.get(1)?,
            asset_type: row.get(2)?,
            auto_tags,
            style: row.get(4)?,
            relevance_score: row.get(5).unwrap_or(0.0),
        })
    })
    .map_err(|e| format!("Query execution failed: {}", e))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("Result collection failed: {}", e))?;

    Ok(results)
}

fn prepare_fts_query(text: &str) -> String {
    let mut query = text.trim().to_string();

    // Add prefix wildcard if no operators
    if !query.contains("OR") && !query.contains("AND") && !query.contains("NOT") {
        query = query
            .split_whitespace()
            .map(|term| format!("{}*", term))
            .collect::<Vec<_>>()
            .join(" ");
    }

    query
}

#[tauri::command]
pub async fn find_similar_assets(
    state: tauri::State<'_, crate::AppState>,
    asset_id: i64,
    limit: u32,
) -> Result<Vec<(i64, f32)>, String> {
    let conn = state.db.lock().unwrap();

    // Get reference embedding
    let embedding_blob: Vec<u8> = conn.query_row(
        "SELECT embedding FROM assets WHERE id = ?",
        params![asset_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("Asset not found: {}", e))?;

    let ref_embedding = deserialize_embedding(&embedding_blob);

    // Get asset type
    let asset_type: String = conn.query_row(
        "SELECT asset_type FROM assets WHERE id = ?",
        params![asset_id],
        |row| row.get(0),
    )?;

    // Get all other embeddings of same type
    let mut stmt = conn.prepare(
        "SELECT id, embedding FROM assets WHERE id != ? AND asset_type = ?"
    )?;

    let results = stmt.query_map(params![asset_id, asset_type], |row| {
        let id: i64 = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        let emb = deserialize_embedding(&blob);
        let similarity = cosine_similarity(&ref_embedding, &emb);

        Ok((id, similarity))
    })?
    .collect::<Result<Vec<_>, _>>()?;

    let mut sorted = results;
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    sorted.truncate(limit as usize);

    Ok(sorted)
}

fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks(4)
        .map(|chunk| {
            if chunk.len() == 4 {
                f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
            } else {
                0.0
            }
        })
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}
```

---

## 5. Frontend: Search UI

**File**: `src/lib/state/search.svelte.ts`

```typescript
import type { Asset } from '$lib/types';
import { invoke } from '@tauri-apps/api/tauri';

export interface SearchFilters {
  text: string;
  assetType?: 'image' | 'audio';
  styles: string[];
  soundCategories: string[];
  durationCategory?: string;
  minWidth?: number;
  minHeight?: number;
  tags: string[];
  isSpritesheet?: boolean;
  sortBy: 'relevance' | 'name' | 'date_modified' | 'date_created';
}

export const searchFilters = $state<SearchFilters>({
  text: '',
  styles: [],
  soundCategories: [],
  tags: [],
  sortBy: 'relevance',
});

export const searchResults = $state<Asset[]>([]);
export const isSearching = $state(false);
export const totalResults = $state(0);

export async function performSearch() {
  if (!searchFilters.text && searchFilters.styles.length === 0) {
    searchResults.length = 0;
    return;
  }

  isSearching = true;
  try {
    const results = await invoke<Asset[]>('search_assets', {
      query: {
        text: searchFilters.text || undefined,
        assetType: searchFilters.assetType,
        styles: searchFilters.styles.length > 0 ? searchFilters.styles : undefined,
        soundCategories: searchFilters.soundCategories.length > 0 ? searchFilters.soundCategories : undefined,
        durationCategory: searchFilters.durationCategory,
        minWidth: searchFilters.minWidth,
        minHeight: searchFilters.minHeight,
        tags: searchFilters.tags.length > 0 ? searchFilters.tags : undefined,
        isSpritesheet: searchFilters.isSpritesheet,
        sortBy: searchFilters.sortBy,
        limit: 50,
        offset: 0,
      },
    });

    searchResults.length = 0;
    searchResults.push(...results);
    totalResults = results.length;
  } catch (error) {
    console.error('Search failed:', error);
  } finally {
    isSearching = false;
  }
}
```

**File**: `src/lib/components/search/SearchBar.svelte`

```svelte
<script lang="ts">
  import { searchFilters, performSearch, isSearching } from '$lib/state/search.svelte';
  import { debounce } from 'lodash-es';

  const debouncedSearch = debounce(performSearch, 300);

  $effect(() => {
    debouncedSearch();
  });

  function handleTextChange(e: Event) {
    const target = e.target as HTMLInputElement;
    searchFilters.text = target.value;
  }

  function toggleStyle(style: string) {
    const idx = searchFilters.styles.indexOf(style);
    if (idx >= 0) {
      searchFilters.styles.splice(idx, 1);
    } else {
      searchFilters.styles.push(style);
    }
    searchFilters.styles = [...searchFilters.styles];
  }

  function toggleCategory(category: string) {
    const idx = searchFilters.soundCategories.indexOf(category);
    if (idx >= 0) {
      searchFilters.soundCategories.splice(idx, 1);
    } else {
      searchFilters.soundCategories.push(category);
    }
    searchFilters.soundCategories = [...searchFilters.soundCategories];
  }
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary rounded-lg">
  <!-- Main search input -->
  <input
    type="text"
    placeholder="Search assets... (e.g., 'pixel art character', 'explosion sound')"
    value={searchFilters.text}
    onchange={handleTextChange}
    disabled={isSearching}
    class="w-full px-4 py-2 border border-default rounded focus:outline-none focus:ring-2 focus:ring-accent"
  />

  <!-- Asset type filter -->
  <select
    bind:value={searchFilters.assetType}
    class="px-4 py-2 border border-default rounded"
  >
    <option value={undefined}>All Assets</option>
    <option value="image">Images Only</option>
    <option value="audio">Audio Only</option>
  </select>

  <!-- Image-specific filters -->
  {#if searchFilters.assetType !== 'audio'}
    <div class="flex gap-2">
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          checked={searchFilters.isSpritesheet}
          onchange={(e) => (searchFilters.isSpritesheet = e.target.checked)}
          class="rounded"
        />
        <span class="text-sm">Sprite Sheets Only</span>
      </label>
    </div>

    <div class="flex flex-wrap gap-2">
      {#each ['pixel_art', 'vector', '3d_render', 'photo'] as style}
        <button
          onclick={() => toggleStyle(style)}
          class={`px-3 py-1 text-sm rounded transition ${
            searchFilters.styles.includes(style)
              ? 'bg-accent text-white'
              : 'bg-secondary border border-default'
          }`}
        >
          {style}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Audio-specific filters -->
  {#if searchFilters.assetType !== 'image'}
    <select
      bind:value={searchFilters.durationCategory}
      class="px-4 py-2 border border-default rounded"
    >
      <option value={undefined}>Any Duration</option>
      <option value="very_short">&lt; 0.5s</option>
      <option value="short">0.5s - 2s</option>
      <option value="medium">2s - 5s</option>
      <option value="long">&gt; 5s</option>
    </select>

    <div class="flex flex-wrap gap-2">
      {#each ['impact', 'ambient', 'voice', 'music', 'effect'] as category}
        <button
          onclick={() => toggleCategory(category)}
          class={`px-3 py-1 text-sm rounded transition ${
            searchFilters.soundCategories.includes(category)
              ? 'bg-accent text-white'
              : 'bg-secondary border border-default'
          }`}
        >
          {category}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Sort options -->
  <select
    bind:value={searchFilters.sortBy}
    class="px-4 py-2 border border-default rounded"
  >
    <option value="relevance">Most Relevant</option>
    <option value="name">Name A-Z</option>
    <option value="date_modified">Recently Modified</option>
    <option value="date_created">Recently Added</option>
  </select>

  {#if isSearching}
    <p class="text-sm text-secondary">Searching...</p>
  {/if}
</div>
```

---

## 6. Preprocessing Pipeline Integration

**File**: `src-tauri/src/commands/process.rs`

```rust
use crate::ml::image::CLIPImageTagger;
use crate::ml::audio::AudioTagger;
use std::path::Path;
use rusqlite::Connection;
use serde_json::json;

pub async fn process_asset(
    conn: &Connection,
    asset_id: i64,
    asset_path: &Path,
    asset_type: &str,
    quality_tier: &str,
) -> Result<(), String> {
    if asset_type == "image" {
        let img = image::open(asset_path)
            .map_err(|e| format!("Failed to load image: {}", e))?;

        // Get CLIP tagger based on tier
        let tagger = match quality_tier {
            "quality" => CLIPImageTagger::new(Path::new("models/clip-vit-l14"))?,
            _ => CLIPImageTagger::new(Path::new("models/clip-vit-b32"))?,
        };

        let tags = tagger.generate_tags(&img).await?;

        // Serialize tags
        let auto_tags_json = serde_json::to_string(&tags.tags)
            .map_err(|e| format!("Serialization failed: {}", e))?;

        // Update database
        conn.execute(
            "UPDATE assets SET auto_tags = ?, style_primary = ?, style_confidence = ?, is_spritesheet = ?, grid_dimensions = ? WHERE id = ?",
            rusqlite::params![
                auto_tags_json,
                tags.style,
                tags.style_confidence,
                tags.is_spritesheet,
                tags.grid_dimensions,
                asset_id
            ],
        ).map_err(|e| format!("Database update failed: {}", e))?;
    }

    Ok(())
}
```

---

## 7. Testing Search

### Verification Checklist

- [ ] FTS5 search returns results matching text query
- [ ] Prefix search works (e.g., "pix" matches "pixel art")
- [ ] Boolean search works ("character AND armor NOT helmet")
- [ ] Style filters show available options
- [ ] Audio category filters work
- [ ] Sprite sheet detection identifies grids correctly
- [ ] Relevance ranking works (best matches first)
- [ ] Similar asset search returns correct embeddings
- [ ] Performance acceptable with 1,000 assets

---

## Next Steps

Once Phase 2 is complete:
1. Move to Phase 3: Advanced Features
2. Implement infinite canvas with PixiJS
3. Add premium tier LLaVA/audio LLMs
4. Implement duplicate detection
