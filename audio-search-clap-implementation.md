# Audio Search Implementation with CLAP

## Executive Summary

Implement advanced text-to-audio search using **CLAP (Contrastive Language-Audio Pretraining)** to enable natural language queries for audio assets. CLAP provides superior text-to-audio alignment compared to PANNs, enabling queries like "footsteps on wood", "distant thunder", or "gentle wind" without being limited to predefined categories.

**Target Performance:**
- **Processing**: 50-120ms per audio file (20-40ms inference + overhead)
- **Total Time**: 10-25 minutes for 10,000 audio files
- **VRAM Usage**: ~1.2GB during processing
- **Search Speed**: <100ms for text-to-audio similarity queries

---

## Why CLAP?

### Comparison with Current System

| Feature | PANNs (Current?) | CLAP (Proposed) |
|---------|------------------|-----------------|
| **Model Type** | Audio classifier | Contrastive text-audio |
| **Text Queries** | ❌ Predefined tags only | ✅ Arbitrary text |
| **Search Quality** | Basic category matching | Semantic understanding |
| **Example Query** | Can't handle "gentle wind" | ✅ Understands nuance |
| **Speed** | 22-45ms/file | 50-120ms/file |
| **VRAM** | 2-3GB | 1.2GB |

### CLAP Advantages

1. **Zero-shot text queries**: "metal impact with reverb", "footstep on concrete", "ambient forest no birds"
2. **Semantic understanding**: Understands relationships between concepts
3. **Better search results**: 75-85% recall vs 65-75% with PANNs
4. **Reasonable speed**: Still processes 10K files in 10-25 minutes

---

## Architecture Overview

### Processing Pipeline

```
Audio File (MP3/WAV/OGG/FLAC)
    ↓
Decode + Resample (Symphonia)
    ↓
Mel Spectrogram (32kHz, 1024 samples)
    ↓
CLAP Audio Encoder (ONNX)
    ↓
512-dim Audio Embedding
    ↓
Store in SQLite (BLOB)
```

### Search Pipeline

```
User Text Query: "footsteps on wood"
    ↓
CLAP Text Encoder (ONNX)
    ↓
512-dim Text Embedding
    ↓
Cosine Similarity vs All Audio Embeddings
    ↓
Ranked Results (Top N)
```

---

## Database Schema Changes

### Option 1: Extend Existing Schema (Recommended)

Add CLAP embeddings alongside existing metadata:

```sql
-- Add CLAP embedding column to audio_metadata
ALTER TABLE audio_metadata ADD COLUMN clap_embedding BLOB;

-- Add processing tier tracking
ALTER TABLE audio_metadata ADD COLUMN processing_tier TEXT DEFAULT 'basic';
-- Values: 'basic' (PANNs only), 'clap' (has CLAP embeddings), 'premium' (LLM descriptions)

-- Add index for processing tier queries
CREATE INDEX idx_audio_processing_tier ON audio_metadata(processing_tier);
```

**Benefits:**
- Preserves existing PANNs data
- Allows incremental upgrade (reprocess files for CLAP)
- Can compare PANNs vs CLAP search quality

### Option 2: Replace PANNs Entirely

If PANNs isn't currently used:

```sql
-- Modify embedding column to store CLAP embeddings
-- Update processing to use CLAP instead of PANNs
```

---

## Rust Backend Implementation

### 1. CLAP Model Integration

**Dependencies (add to `Cargo.toml`):**

```toml
[dependencies]
# Existing audio dependencies
rodio = { version = "0.21", default-features = false, features = ["symphonia-all"] }
symphonia = "0.5"

# ML inference
ort = { version = "2.0.0-rc.10", features = ["cuda", "load-dynamic"] }

# Audio processing
rubato = "0.15"  # Resampling
hound = "3.5"    # WAV encoding
```

**Model Manager (`src-tauri/src/audio/clap_model.rs`):**

```rust
use ort::{Session, SessionBuilder, Value};
use ndarray::{Array1, Array2, ArrayView1};
use std::path::Path;

pub struct ClapModel {
    audio_encoder: Session,
    text_encoder: Session,
}

impl ClapModel {
    /// Load CLAP ONNX models
    pub fn new(model_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let audio_path = model_dir.join("clap_audio_encoder.onnx");
        let text_path = model_dir.join("clap_text_encoder.onnx");

        let audio_encoder = SessionBuilder::new()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(audio_path)?;

        let text_encoder = SessionBuilder::new()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(text_path)?;

        Ok(Self {
            audio_encoder,
            text_encoder,
        })
    }

    /// Generate audio embedding from mel spectrogram
    pub fn encode_audio(&self, mel_spec: &Array2<f32>) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        // Input shape: [batch=1, mels=64, time_frames]
        let input = mel_spec.view().insert_axis(ndarray::Axis(0));

        let input_value = Value::from_array(input)?;
        let outputs = self.audio_encoder.run(ort::inputs![input_value]?)?;

        // Extract embedding (512-dim vector)
        let embedding: ArrayView1<f32> = outputs[0].try_extract_tensor()?;

        Ok(embedding.to_vec())
    }

    /// Generate text embedding from query string
    pub fn encode_text(&self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        // Tokenize text (simplified - production should use proper tokenizer)
        let tokens = self.tokenize(text)?;

        let input_value = Value::from_array(tokens.view())?;
        let outputs = self.text_encoder.run(ort::inputs![input_value]?)?;

        // Extract embedding (512-dim vector)
        let embedding: ArrayView1<f32> = outputs[0].try_extract_tensor()?;

        Ok(embedding.to_vec())
    }

    /// Simplified tokenization (replace with proper CLIP tokenizer)
    fn tokenize(&self, text: &str) -> Result<Array1<i64>, Box<dyn std::error::Error>> {
        // TODO: Implement proper CLIP text tokenization
        // For now, placeholder that should be replaced with:
        // - Lowercase text
        // - BPE tokenization
        // - Special tokens ([CLS], [SEP])
        // - Padding to max_length (77 tokens for CLIP)

        unimplemented!("Use proper CLIP tokenizer library")
    }
}
```

### 2. Audio Processing Pipeline

**Mel Spectrogram Generation (`src-tauri/src/audio/features.rs`):**

```rust
use ndarray::{Array1, Array2};
use std::f32::consts::PI;

pub struct MelConfig {
    pub sample_rate: u32,
    pub n_fft: usize,
    pub hop_length: usize,
    pub n_mels: usize,
    pub fmin: f32,
    pub fmax: f32,
}

impl Default for MelConfig {
    fn default() -> Self {
        Self {
            sample_rate: 32000,  // CLAP expects 32kHz
            n_fft: 1024,
            hop_length: 320,
            n_mels: 64,
            fmin: 0.0,
            fmax: 16000.0,
        }
    }
}

/// Generate mel spectrogram from audio samples
pub fn create_mel_spectrogram(
    samples: &[f32],
    config: &MelConfig,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    // 1. Apply STFT (Short-Time Fourier Transform)
    let stft = compute_stft(samples, config.n_fft, config.hop_length)?;

    // 2. Compute power spectrogram
    let power_spec = stft.mapv(|c| c.norm_sqr());

    // 3. Create mel filterbank
    let mel_filters = create_mel_filterbank(
        config.n_mels,
        config.n_fft,
        config.sample_rate,
        config.fmin,
        config.fmax,
    )?;

    // 4. Apply mel filters
    let mel_spec = mel_filters.dot(&power_spec);

    // 5. Convert to log scale
    let log_mel = mel_spec.mapv(|x| (x + 1e-10).ln());

    Ok(log_mel)
}

fn compute_stft(
    samples: &[f32],
    n_fft: usize,
    hop_length: usize,
) -> Result<Array2<num_complex::Complex<f32>>, Box<dyn std::error::Error>> {
    // TODO: Implement STFT
    // Libraries: rustfft, realfft
    unimplemented!("Implement STFT")
}

fn create_mel_filterbank(
    n_mels: usize,
    n_fft: usize,
    sample_rate: u32,
    fmin: f32,
    fmax: f32,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    // TODO: Implement mel filterbank creation
    // Convert Hz to mel scale, create triangular filters
    unimplemented!("Implement mel filterbank")
}
```

**Resampling Audio (`src-tauri/src/audio/resample.rs`):**

```rust
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};

/// Resample audio to target sample rate
pub fn resample_audio(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        to_rate as f64 / from_rate as f64,
        2.0,
        params,
        samples.len(),
        1, // mono
    )?;

    let waves_in = vec![samples.to_vec()];
    let waves_out = resampler.process(&waves_in, None)?;

    Ok(waves_out[0].clone())
}
```

### 3. Processing Command Integration

**Update `src-tauri/src/commands/process.rs`:**

```rust
use crate::audio::clap_model::ClapModel;
use crate::audio::features::{create_mel_spectrogram, MelConfig};
use crate::audio::resample::resample_audio;

// Add CLAP model to processor state
pub struct AudioProcessor {
    clap_model: ClapModel,
    mel_config: MelConfig,
}

impl AudioProcessor {
    pub fn new(model_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let clap_model = ClapModel::new(model_dir)?;
        let mel_config = MelConfig::default();

        Ok(Self {
            clap_model,
            mel_config,
        })
    }

    /// Process single audio file
    pub async fn process_audio_file(
        &self,
        file_path: &Path,
    ) -> Result<AudioProcessingResult, Box<dyn std::error::Error>> {
        // 1. Load and decode audio
        let (samples, sample_rate, channels) = load_audio(file_path)?;

        // 2. Convert to mono if stereo
        let mono_samples = if channels == 2 {
            to_mono(&samples)
        } else {
            samples
        };

        // 3. Resample to 32kHz (CLAP requirement)
        let resampled = resample_audio(
            &mono_samples,
            sample_rate,
            self.mel_config.sample_rate,
        )?;

        // 4. Generate mel spectrogram
        let mel_spec = create_mel_spectrogram(&resampled, &self.mel_config)?;

        // 5. Generate CLAP embedding
        let embedding = self.clap_model.encode_audio(&mel_spec)?;

        // 6. Extract basic metadata
        let duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as i64;

        Ok(AudioProcessingResult {
            duration_ms,
            sample_rate,
            channels: channels as i32,
            clap_embedding: embedding,
        })
    }
}

pub struct AudioProcessingResult {
    pub duration_ms: i64,
    pub sample_rate: u32,
    pub channels: i32,
    pub clap_embedding: Vec<f32>,
}

fn load_audio(path: &Path) -> Result<(Vec<f32>, u32, usize), Box<dyn std::error::Error>> {
    // Use Symphonia to decode various audio formats
    // TODO: Implement proper audio loading
    unimplemented!("Implement audio loading with Symphonia")
}

fn to_mono(stereo: &[f32]) -> Vec<f32> {
    stereo
        .chunks_exact(2)
        .map(|frame| (frame[0] + frame[1]) / 2.0)
        .collect()
}
```

**Update Database Write:**

```rust
// In batch processing function
async fn save_audio_metadata(
    pool: &sqlx::SqlitePool,
    asset_id: i64,
    result: &AudioProcessingResult,
) -> Result<(), sqlx::Error> {
    // Serialize embedding to bytes
    let embedding_bytes: Vec<u8> = result.clap_embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();

    sqlx::query(
        "INSERT INTO audio_metadata
         (asset_id, duration_ms, sample_rate, channels, clap_embedding, processing_tier)
         VALUES (?, ?, ?, ?, ?, 'clap')"
    )
    .bind(asset_id)
    .bind(result.duration_ms)
    .bind(result.sample_rate as i64)
    .bind(result.channels)
    .bind(&embedding_bytes)
    .execute(pool)
    .await?;

    Ok(())
}
```

---

## Frontend Implementation

### 1. Database Queries (`src/lib/database/queries.ts`)

```typescript
import type Database from '@tauri-apps/plugin-sql';

export interface AudioSearchResult {
  id: number;
  name: string;
  path: string;
  duration_ms: number;
  sample_rate: number;
  channels: number;
  similarity: number; // Cosine similarity score
}

/**
 * Search audio files using text query with CLAP embeddings
 */
export async function searchAudioByText(
  db: Database,
  textQuery: string,
  limit: number = 50
): Promise<AudioSearchResult[]> {
  // This requires backend command - can't compute CLAP text embedding in frontend
  // See searchAudioByTextCommand below
  throw new Error('Use searchAudioByTextCommand instead');
}

/**
 * Get audio files with CLAP embeddings (for manual similarity calculation)
 */
export async function getAudioWithEmbeddings(
  db: Database,
  limit?: number,
  offset: number = 0
): Promise<Array<{ id: number; name: string; embedding: Uint8Array }>> {
  const query = `
    SELECT
      a.id,
      a.name,
      am.clap_embedding
    FROM assets a
    INNER JOIN audio_metadata am ON a.id = am.asset_id
    WHERE a.asset_type = 'audio'
      AND am.clap_embedding IS NOT NULL
    ORDER BY a.id
    ${limit ? `LIMIT ${limit}` : ''}
    OFFSET ${offset}
  `;

  const results = await db.select<Array<{
    id: number;
    name: string;
    clap_embedding: number[];
  }>>(query);

  return results.map(row => ({
    id: row.id,
    name: row.name,
    embedding: new Uint8Array(row.clap_embedding),
  }));
}

/**
 * Get count of audio files by processing tier
 */
export async function getAudioProcessingStats(
  db: Database
): Promise<{ total: number; with_clap: number; pending: number }> {
  const result = await db.select<Array<{
    total: number;
    with_clap: number;
  }>>(
    `SELECT
       COUNT(*) as total,
       SUM(CASE WHEN am.processing_tier = 'clap' THEN 1 ELSE 0 END) as with_clap
     FROM assets a
     LEFT JOIN audio_metadata am ON a.id = am.asset_id
     WHERE a.asset_type = 'audio'`
  );

  const stats = result[0];
  return {
    total: stats.total,
    with_clap: stats.with_clap,
    pending: stats.total - stats.with_clap,
  };
}
```

### 2. Backend Search Command

**Add to `src-tauri/src/commands/search.rs`:**

```rust
use crate::audio::clap_model::ClapModel;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct AudioSearchResult {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub duration_ms: i64,
    pub sample_rate: i64,
    pub channels: i32,
    pub similarity: f32,
}

/// Search audio files using natural language text query
#[tauri::command]
pub async fn search_audio_by_text(
    state: tauri::State<'_, AppState>,
    text_query: String,
    limit: Option<usize>,
) -> Result<Vec<AudioSearchResult>, String> {
    let limit = limit.unwrap_or(50);

    // 1. Generate text embedding using CLAP
    let text_embedding = state.clap_model
        .encode_text(&text_query)
        .map_err(|e| format!("Failed to encode text: {}", e))?;

    // 2. Load all audio embeddings from database
    let pool = &state.db_pool;
    let audio_data: Vec<(i64, String, String, i64, i64, i32, Vec<u8>)> = sqlx::query_as(
        "SELECT
           a.id, a.name, a.path,
           am.duration_ms, am.sample_rate, am.channels,
           am.clap_embedding
         FROM assets a
         INNER JOIN audio_metadata am ON a.id = am.asset_id
         WHERE a.asset_type = 'audio'
           AND am.clap_embedding IS NOT NULL"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    // 3. Calculate cosine similarity for each audio file
    let mut results: Vec<AudioSearchResult> = audio_data
        .into_iter()
        .map(|(id, name, path, duration_ms, sample_rate, channels, embedding_bytes)| {
            // Deserialize embedding from bytes
            let audio_embedding = deserialize_embedding(&embedding_bytes);

            // Calculate cosine similarity
            let similarity = cosine_similarity(&text_embedding, &audio_embedding);

            AudioSearchResult {
                id,
                name,
                path,
                duration_ms,
                sample_rate,
                channels,
                similarity,
            }
        })
        .collect();

    // 4. Sort by similarity (descending) and return top N
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    results.truncate(limit);

    Ok(results)
}

/// Deserialize f32 embedding from byte array
fn deserialize_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Calculate cosine similarity between two embeddings
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}
```

### 3. Frontend Search Component

**Create `src/lib/components/AudioSearch.svelte`:**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { showToast } from '$lib/state/ui.svelte';

  interface AudioSearchResult {
    id: number;
    name: string;
    path: string;
    duration_ms: number;
    similarity: number;
  }

  let searchQuery = $state('');
  let results = $state<AudioSearchResult[]>([]);
  let isSearching = $state(false);

  async function handleSearch() {
    if (!searchQuery.trim()) {
      results = [];
      return;
    }

    isSearching = true;
    try {
      const searchResults = await invoke<AudioSearchResult[]>('search_audio_by_text', {
        textQuery: searchQuery,
        limit: 50,
      });

      results = searchResults;

      if (searchResults.length === 0) {
        showToast('No matching audio found', 'info');
      }
    } catch (error) {
      showToast(`Search failed: ${error}`, 'error');
      console.error('Audio search error:', error);
    } finally {
      isSearching = false;
    }
  }

  function formatDuration(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return minutes > 0
      ? `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`
      : `${seconds}s`;
  }
</script>

<div class="flex flex-col gap-4 p-4">
  <!-- Search Input -->
  <div class="flex gap-2">
    <input
      type="text"
      bind:value={searchQuery}
      onkeydown={(e) => e.key === 'Enter' && handleSearch()}
      placeholder="Search audio... (e.g., 'footsteps on wood', 'distant thunder')"
      class="flex-1 px-4 py-2 bg-primary border border-default rounded-lg text-primary
             placeholder:text-secondary focus:outline-none focus:ring-2 focus:ring-accent"
    />
    <button
      onclick={handleSearch}
      disabled={isSearching || !searchQuery.trim()}
      class="px-6 py-2 bg-accent text-white rounded-lg font-medium
             hover:bg-accent/90 disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {isSearching ? 'Searching...' : 'Search'}
    </button>
  </div>

  <!-- Example Queries -->
  <div class="flex gap-2 flex-wrap">
    <span class="text-sm text-secondary">Try:</span>
    {#each ['footsteps on wood', 'explosion sound', 'gentle wind', 'door closing'] as example}
      <button
        onclick={() => {
          searchQuery = example;
          handleSearch();
        }}
        class="px-3 py-1 text-sm bg-secondary border border-default rounded-full
               hover:bg-accent hover:text-white transition-colors"
      >
        {example}
      </button>
    {/each}
  </div>

  <!-- Results -->
  {#if results.length > 0}
    <div class="flex flex-col gap-2">
      <h3 class="text-sm font-semibold text-secondary">
        {results.length} results found
      </h3>

      {#each results as result (result.id)}
        <div
          class="flex items-center justify-between p-3 bg-secondary border border-default rounded-lg
                 hover:border-accent transition-colors"
        >
          <div class="flex flex-col gap-1">
            <span class="font-medium text-primary">{result.name}</span>
            <span class="text-xs text-secondary">
              Duration: {formatDuration(result.duration_ms)}
            </span>
          </div>

          <div class="flex items-center gap-3">
            <span class="text-sm text-secondary">
              {(result.similarity * 100).toFixed(1)}% match
            </span>
            <button
              class="px-3 py-1 text-sm bg-accent text-white rounded
                     hover:bg-accent/90 transition-colors"
              onclick={() => {
                // TODO: Play audio preview
                console.log('Play audio:', result.id);
              }}
            >
              Play
            </button>
          </div>
        </div>
      {/each}
    </div>
  {:else if searchQuery && !isSearching}
    <p class="text-center text-secondary py-8">No results found</p>
  {/if}
</div>
```

---

## Model Acquisition & Setup

### CLAP Model Files

**Required Files:**
1. `clap_audio_encoder.onnx` (~300MB) - Audio encoder
2. `clap_text_encoder.onnx` (~330MB) - Text encoder
3. `tokenizer.json` - CLIP BPE tokenizer

**Sources:**
- **Official**: [LAION-CLAP on Hugging Face](https://huggingface.co/laion/clap-htsat-fused)
- **Microsoft CLAP**: [microsoft/msclap](https://github.com/microsoft/CLAP)

**Conversion to ONNX:**

```python
# Convert PyTorch CLAP to ONNX
import torch
from transformers import ClapModel

model = ClapModel.from_pretrained("laion/clap-htsat-fused")

# Export audio encoder
dummy_audio = torch.randn(1, 1, 1024, 64)  # [batch, channels, time, mels]
torch.onnx.export(
    model.audio_model,
    dummy_audio,
    "clap_audio_encoder.onnx",
    input_names=["audio"],
    output_names=["embedding"],
    dynamic_axes={"audio": {0: "batch", 2: "time"}},
)

# Export text encoder
dummy_text = torch.randint(0, 49408, (1, 77))  # [batch, seq_len]
torch.onnx.export(
    model.text_model,
    dummy_text,
    "clap_text_encoder.onnx",
    input_names=["input_ids"],
    output_names=["embedding"],
    dynamic_axes={"input_ids": {0: "batch"}},
)
```

**Storage Location:**
- Store models in app data directory: `$APPDATA/asseteer/models/clap/`
- Check for models on startup, prompt user to download if missing

---

## Performance Optimization

### 1. Batch Processing

```rust
// Process audio files in parallel batches
pub async fn process_audio_batch(
    processor: &AudioProcessor,
    file_paths: Vec<PathBuf>,
) -> Vec<Result<AudioProcessingResult, String>> {
    use rayon::prelude::*;

    file_paths
        .par_iter()
        .map(|path| {
            processor.process_audio_file(path)
                .map_err(|e| e.to_string())
        })
        .collect()
}
```

### 2. Embedding Search Optimization

For large audio libraries (10K+ files), consider:

**Option A: In-Memory Cache (Simple)**
```rust
// Load all embeddings into memory on startup
pub struct EmbeddingCache {
    embeddings: HashMap<i64, Vec<f32>>,
}

// Search: O(n) linear scan with SIMD-optimized cosine similarity
```

**Option B: Vector Database (Advanced)**
- Use [qdrant](https://github.com/qdrant/qdrant) or [milvus](https://milvus.io/)
- Approximate nearest neighbor search
- Sub-10ms search for 100K+ vectors
- Overkill for <50K audio files

### 3. ONNX Runtime Optimization

```rust
// Enable GPU acceleration if available
let audio_encoder = SessionBuilder::new()?
    .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
    .with_execution_providers([
        ort::ExecutionProvider::CUDA(Default::default()),  // Try GPU first
        ort::ExecutionProvider::CPU(Default::default()),   // Fallback to CPU
    ])?
    .commit_from_file(audio_path)?;
```

---

## Testing Strategy

### Unit Tests

**Test CLAP Model Loading:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clap_model_loading() {
        let model_dir = Path::new("test_models");
        let clap = ClapModel::new(model_dir).expect("Failed to load model");
        // Assert model loaded successfully
    }

    #[test]
    fn test_text_encoding() {
        let clap = setup_test_model();
        let embedding = clap.encode_text("footsteps").unwrap();

        assert_eq!(embedding.len(), 512);  // CLAP embedding dimension
        assert!(embedding.iter().all(|&x| x.is_finite()));  // No NaN/Inf
    }
}
```

**Test Audio Processing:**
```rust
#[tokio::test]
async fn test_audio_processing() {
    let processor = AudioProcessor::new(Path::new("models")).unwrap();
    let test_audio = Path::new("test_assets/test_sound.wav");

    let result = processor.process_audio_file(test_audio).await.unwrap();

    assert_eq!(result.clap_embedding.len(), 512);
    assert!(result.duration_ms > 0);
}
```

### Integration Tests

**Test Search Pipeline:**
```rust
#[tokio::test]
async fn test_search_flow() {
    // 1. Process test audio files
    let files = vec!["footstep.wav", "thunder.wav", "wind.wav"];
    // ... process files ...

    // 2. Search with text query
    let results = search_audio_by_text("footsteps on wood", Some(10)).await.unwrap();

    // 3. Verify footstep.wav ranks highest
    assert_eq!(results[0].name, "footstep.wav");
    assert!(results[0].similarity > 0.6);  // Strong match threshold
}
```

### Manual Testing Checklist

- [ ] Process 100 audio files successfully
- [ ] Search queries return relevant results:
  - [ ] "footsteps on wood"
  - [ ] "explosion sound"
  - [ ] "gentle wind"
  - [ ] "door closing"
  - [ ] "metal impact"
- [ ] Search completes in <200ms for 10K audio library
- [ ] Processing shows accurate progress updates
- [ ] Error handling for corrupted audio files
- [ ] Memory usage stays under 2GB during processing

---

## Implementation Phases

### Phase 1: Core CLAP Integration (Week 1)
**Goal:** Get CLAP model working in Rust

- [ ] Add ONNX Runtime dependencies to Cargo.toml
- [ ] Implement `ClapModel` struct with audio/text encoders
- [ ] Create mel spectrogram generation pipeline
- [ ] Add audio resampling (to 32kHz)
- [ ] Write unit tests for embedding generation
- [ ] Verify embeddings match expected dimensions (512-dim)

**Deliverable:** Rust function that takes audio file → returns 512-dim embedding

### Phase 2: Database & Processing (Week 1-2)
**Goal:** Integrate CLAP into existing processing pipeline

- [ ] Update database schema (add `clap_embedding` column)
- [ ] Modify `AudioProcessor` to generate CLAP embeddings
- [ ] Implement batch processing for audio files
- [ ] Add progress tracking for CLAP processing
- [ ] Test processing pipeline with 100-1000 audio files
- [ ] Benchmark processing speed (target: 50-120ms per file)

**Deliverable:** Audio files processed with CLAP embeddings stored in database

### Phase 3: Text Search Backend (Week 2)
**Goal:** Implement text-to-audio search

- [ ] Implement text tokenization (CLIP BPE tokenizer)
- [ ] Create `search_audio_by_text` Tauri command
- [ ] Implement cosine similarity calculation
- [ ] Add result ranking and filtering
- [ ] Optimize search for 10K+ audio library
- [ ] Write integration tests for search

**Deliverable:** Backend API for text-based audio search

### Phase 4: Frontend Integration (Week 2)
**Goal:** User-facing search interface

- [ ] Create `AudioSearch.svelte` component
- [ ] Implement search input with debouncing
- [ ] Display ranked search results with similarity scores
- [ ] Add audio preview playback
- [ ] Show example queries for discoverability
- [ ] Add loading states and error handling

**Deliverable:** Working audio search UI in application

### Phase 5: Polish & Optimization (Week 3)
**Goal:** Production-ready performance

- [ ] Profile and optimize hot paths
- [ ] Implement embedding caching
- [ ] Add CUDA/GPU acceleration if available
- [ ] Optimize database queries
- [ ] Add comprehensive error handling
- [ ] Write user documentation
- [ ] Performance testing with 10K+ audio files

**Deliverable:** Production-ready audio search feature

---

## Success Metrics

### Performance Targets
- ✅ **Processing Speed**: 50-120ms per audio file
- ✅ **Search Latency**: <200ms for text queries (10K audio library)
- ✅ **VRAM Usage**: <1.5GB during processing
- ✅ **RAM Usage**: <2GB for embedding cache (10K files)

### Quality Targets
- ✅ **Search Precision**: >80% relevant results in top 10
- ✅ **Query Understanding**: Handles arbitrary text descriptions
- ✅ **Example Success**: Query "footsteps on wood" ranks footstep sounds in top 5

### User Experience
- ✅ **Search Response**: Results appear in <1 second
- ✅ **Processing Feedback**: Real-time progress updates
- ✅ **Error Handling**: Clear error messages for failures
- ✅ **Discoverability**: Example queries help users understand capabilities

---

## Risks & Mitigations

### Risk 1: CLAP Model Size & Download
**Problem:** 630MB model download is large for users

**Mitigations:**
- Make CLAP optional (offer "basic" vs "advanced" search modes)
- Download on-demand when user enables advanced search
- Show download progress with pause/resume support
- Cache models in app data directory

### Risk 2: ONNX Runtime Dependencies
**Problem:** ONNX Runtime may have platform-specific issues

**Mitigations:**
- Test on Windows, macOS, Linux before release
- Bundle ONNX Runtime libraries with app
- Provide fallback to CPU if GPU fails
- Clear error messages if model loading fails

### Risk 3: Tokenizer Implementation
**Problem:** CLIP BPE tokenizer is complex to implement in Rust

**Mitigations:**
- Use existing Rust tokenizer libraries (tokenizers.rs)
- Port Python implementation if needed
- Pre-tokenize common queries as fallback
- Document tokenizer requirements clearly

### Risk 4: Search Performance at Scale
**Problem:** Linear scan may be slow for 50K+ audio files

**Mitigations:**
- Profile with realistic dataset sizes
- Implement SIMD-optimized cosine similarity
- Consider approximate nearest neighbor search if needed
- Cache embeddings in memory

---

## Future Enhancements

### Phase 2 Features (Post-MVP)

**1. Hybrid Search**
Combine text search with tag filtering:
```typescript
searchAudioByText(
  query: "footsteps",
  filters: { duration_category: "short", tags: ["game"] }
)
```

**2. Audio-to-Audio Search**
"Find sounds similar to this one":
```rust
#[tauri::command]
pub async fn find_similar_audio(
    reference_audio_id: i64,
    limit: usize,
) -> Result<Vec<AudioSearchResult>, String>
```

**3. Search History & Suggestions**
- Track popular queries
- Suggest related searches
- Auto-complete based on history

**4. Advanced Filters**
- Sample rate range
- Duration range
- Channel count (mono/stereo)
- Processing tier (basic/CLAP/premium)

**5. Batch Operations**
- Tag multiple search results at once
- Export search results to playlist
- Bulk metadata editing

---

## References

### Documentation
- [CLAP Paper (LAION)](https://arxiv.org/abs/2211.06687)
- [ONNX Runtime Rust Docs](https://docs.rs/ort/latest/ort/)
- [Symphonia Audio Decoder](https://github.com/pdeljanov/Symphonia)

### Example Implementations
- [CLAP Official Repo](https://github.com/LAION-AI/CLAP)
- [Microsoft CLAP](https://github.com/microsoft/CLAP)
- [Hugging Face Transformers CLAP](https://huggingface.co/docs/transformers/model_doc/clap)

### Related Tools
- [Freesound API](https://freesound.org/docs/api/) - Similar text-to-audio search
- [AudioSet](https://research.google.com/audioset/) - Audio classification dataset

---

## Conclusion

Implementing CLAP-based audio search provides **significant improvements** over tag-based search:

**Key Benefits:**
- ✅ Natural language queries ("footsteps on wood floor")
- ✅ Zero-shot learning (no predefined categories needed)
- ✅ Semantic understanding (knows "explosion" relates to "blast", "boom")
- ✅ Reasonable performance (50-120ms per file, <200ms search)

**Implementation Effort:**
- **3 weeks** for full implementation
- **Moderate complexity** (audio processing + ML integration)
- **Well-documented** CLAP models and ONNX tooling
- **Clear path** from MVP to production-ready

This feature will dramatically improve audio asset discoverability compared to filename/tag-based search, making it easier for users to find exactly the sounds they need using natural language descriptions.
