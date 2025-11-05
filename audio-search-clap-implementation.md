# Audio Search Implementation with CLAP

## Executive Summary

Implement advanced text-to-audio search using **CLAP (Contrastive Language-Audio Pretraining)** to enable natural language queries for audio assets. CLAP provides content-based semantic search, enabling queries like "footsteps on wood", "distant thunder", or "gentle wind" that understand audio content rather than just matching filenames.

**Target Performance:**
- **Processing**: 50-120ms per audio file (20-40ms inference + overhead)
- **Total Time**: 10-25 minutes for 10,000 audio files (3-5 hours for 200,000 files)
- **VRAM Usage**: ~1.2GB during processing
- **Search Speed**: ~500ms for 200K files (faster with duration/path filters)

**Note on T-CLAP:** T-CLAP offers enhanced temporal understanding but has less mature tooling. CLAP is recommended for initial implementation, with T-CLAP as potential future enhancement if temporal queries become important.

---

## Why CLAP?

### Comparison with Current System

| Feature | Current (FTS5 Filename) | CLAP (Proposed) |
|---------|-------------------------|-----------------|
| **Search Type** | Filename text matching | Content-based semantic search |
| **Text Queries** | ❌ Must match filename | ✅ Natural language queries |
| **Example** | "footstep.wav" ✅ / "footsteps on wood" ❌ | Both work! ✅ |
| **Understanding** | None (text matching only) | Semantic audio understanding |
| **Processing** | None required | 50-120ms per file |

### CLAP Advantages

1. **Zero-shot text queries**: "metal impact with reverb", "footstep on concrete", "ambient forest no birds"
2. **Semantic understanding**: Understands relationships between concepts (e.g., "explosion" relates to "blast", "boom")
3. **Content-based**: Searches actual audio content, not just filenames
4. **Handles long-form**: Works with both sound effects and full music tracks

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

## Database Schema

**Note:** Since we can start with a fresh database, no migrations needed - just define the schema directly.

### Audio Metadata Table

```sql
CREATE TABLE audio_metadata (
    asset_id INTEGER PRIMARY KEY,
    duration_ms INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL,
    channels INTEGER NOT NULL,
    clap_embedding BLOB,  -- 512 floats × 4 bytes = 2048 bytes per file
    FOREIGN KEY (asset_id) REFERENCES assets(id)
);

-- Index for duration-based filtering (useful for 200K+ file search)
CREATE INDEX idx_audio_duration ON audio_metadata(duration_ms);
```

**Storage Considerations:**
- **10K files**: ~20MB embeddings
- **200K files**: ~400MB embeddings
- Linear scan acceptable with pre-filtering by duration/path

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
tokenizers = "0.15"  # CLIP BPE tokenizer

# Audio processing
rubato = "0.15"  # Resampling
rustfft = "6.1"  # FFT for STFT
ndarray = "0.15" # Array operations
num-complex = "0.4"  # Complex numbers for FFT
```

**Model Manager (`src-tauri/src/audio/clap_model.rs`):**

```rust
use ort::{Session, SessionBuilder, Value};
use ndarray::{Array1, Array2, ArrayView1};
use tokenizers::Tokenizer;
use std::path::Path;

pub struct ClapModel {
    audio_encoder: Session,
    text_encoder: Session,
    tokenizer: Tokenizer,
}

impl ClapModel {
    /// Load CLAP ONNX models and tokenizer
    pub fn new(model_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let audio_path = model_dir.join("clap_audio_encoder.onnx");
        let text_path = model_dir.join("clap_text_encoder.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        let audio_encoder = SessionBuilder::new()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(audio_path)?;

        let text_encoder = SessionBuilder::new()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(text_path)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)?;

        Ok(Self {
            audio_encoder,
            text_encoder,
            tokenizer,
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
        // Tokenize text using CLIP tokenizer
        let encoding = self.tokenizer.encode(text, false)?;
        let token_ids = encoding.get_ids();

        // Convert to i64 array and pad/truncate to 77 tokens (CLIP standard)
        let mut tokens = vec![0i64; 77];
        for (i, &id) in token_ids.iter().enumerate().take(77) {
            tokens[i] = id as i64;
        }

        let tokens_array = Array1::from_vec(tokens).insert_axis(ndarray::Axis(0));
        let input_value = Value::from_array(tokens_array.view())?;
        let outputs = self.text_encoder.run(ort::inputs![input_value]?)?;

        // Extract embedding (512-dim vector)
        let embedding: ArrayView1<f32> = outputs[0].try_extract_tensor()?;

        Ok(embedding.to_vec())
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
    use rustfft::{FftPlanner, num_complex::Complex};

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n_fft);

    let num_frames = (samples.len() - n_fft) / hop_length + 1;
    let mut stft_result = Array2::zeros((n_fft / 2 + 1, num_frames));

    // Apply Hann window
    let window: Vec<f32> = (0..n_fft)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / n_fft as f32).cos()))
        .collect();

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop_length;
        let mut buffer: Vec<Complex<f32>> = samples[start..start + n_fft]
            .iter()
            .zip(window.iter())
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();

        fft.process(&mut buffer);

        // Keep only positive frequencies
        for (freq_idx, value) in buffer[..(n_fft / 2 + 1)].iter().enumerate() {
            stft_result[[freq_idx, frame_idx]] = *value;
        }
    }

    Ok(stft_result)
}

fn create_mel_filterbank(
    n_mels: usize,
    n_fft: usize,
    sample_rate: u32,
    fmin: f32,
    fmax: f32,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    // Helper: Convert Hz to mel scale
    let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
    let mel_to_hz = |mel: f32| 700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0);

    let mel_min = hz_to_mel(fmin);
    let mel_max = hz_to_mel(fmax);
    let mel_points: Vec<f32> = (0..=n_mels + 1)
        .map(|i| mel_to_hz(mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32))
        .collect();

    let n_freqs = n_fft / 2 + 1;
    let fft_freqs: Vec<f32> = (0..n_freqs)
        .map(|i| i as f32 * sample_rate as f32 / n_fft as f32)
        .collect();

    let mut filterbank = Array2::zeros((n_mels, n_freqs));

    for mel_idx in 0..n_mels {
        let left = mel_points[mel_idx];
        let center = mel_points[mel_idx + 1];
        let right = mel_points[mel_idx + 2];

        for (freq_idx, &freq) in fft_freqs.iter().enumerate() {
            if freq >= left && freq <= center {
                filterbank[[mel_idx, freq_idx]] = (freq - left) / (center - left);
            } else if freq > center && freq <= right {
                filterbank[[mel_idx, freq_idx]] = (right - freq) / (right - center);
            }
        }
    }

    Ok(filterbank)
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
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    use std::fs::File;

    // Open file and create media source
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Probe format
    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(&ext.to_string_lossy());
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;

    let mut format = probed.format;
    let track = format.default_track()
        .ok_or("No audio track found")?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())?;

    let sample_rate = track.codec_params.sample_rate
        .ok_or("Sample rate not found")? as u32;
    let channels = track.codec_params.channels
        .ok_or("Channel count not found")?
        .count();

    let mut samples = Vec::new();

    // Decode all packets
    while let Ok(packet) = format.next_packet() {
        let decoded = decoder.decode(&packet)?;

        let spec = *decoded.spec();
        let duration = decoded.capacity() as u64;

        let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
        sample_buf.copy_interleaved_ref(decoded);

        samples.extend_from_slice(sample_buf.samples());
    }

    Ok((samples, sample_rate, channels))
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
         (asset_id, duration_ms, sample_rate, channels, clap_embedding)
         VALUES (?, ?, ?, ?, ?)"
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

**Note:** Application already has existing Tauri + Svelte UI infrastructure. Integration points below show how to add audio search to existing interface.

### 1. Database Queries (`src/lib/database/queries.ts`)

```typescript
import type Database from '@tauri-apps/plugin-sql';

/**
 * Get count of audio files with CLAP embeddings
 */
export async function getAudioWithClapCount(
  db: Database
): Promise<{ total: number; with_clap: number; pending: number }> {
  const result = await db.select<Array<{
    total: number;
    with_clap: number;
  }>>(
    `SELECT
       COUNT(*) as total,
       SUM(CASE WHEN am.clap_embedding IS NOT NULL THEN 1 ELSE 0 END) as with_clap
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

/// Search audio files using natural language text query with optional filters
#[tauri::command]
pub async fn search_audio_by_text(
    state: tauri::State<'_, AppState>,
    text_query: String,
    duration_min: Option<i64>,
    duration_max: Option<i64>,
    path_filter: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AudioSearchResult>, String> {
    use rayon::prelude::*;

    let limit = limit.unwrap_or(50);

    // 1. Generate text embedding using CLAP
    let text_embedding = state.clap_model
        .encode_text(&text_query)
        .map_err(|e| format!("Failed to encode text: {}", e))?;

    // 2. Build filtered query
    let mut query = String::from(
        "SELECT a.id, a.name, a.path, am.duration_ms, am.sample_rate, am.channels, am.clap_embedding
         FROM assets a
         INNER JOIN audio_metadata am ON a.id = am.asset_id
         WHERE a.asset_type = 'audio' AND am.clap_embedding IS NOT NULL"
    );

    if duration_min.is_some() {
        query.push_str(&format!(" AND am.duration_ms >= {}", duration_min.unwrap()));
    }
    if duration_max.is_some() {
        query.push_str(&format!(" AND am.duration_ms <= {}", duration_max.unwrap()));
    }
    if let Some(ref path) = path_filter {
        query.push_str(&format!(" AND a.path LIKE '%{}%'", path));
    }

    let pool = &state.db_pool;
    let audio_data: Vec<(i64, String, String, i64, i64, i32, Vec<u8>)> =
        sqlx::query_as(&query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    // 3. Calculate cosine similarity in parallel
    let mut results: Vec<AudioSearchResult> = audio_data
        .par_iter()
        .map(|(id, name, path, duration_ms, sample_rate, channels, embedding_bytes)| {
            let audio_embedding = deserialize_embedding(embedding_bytes);
            let similarity = cosine_similarity(&text_embedding, &audio_embedding);

            AudioSearchResult {
                id: *id,
                name: name.clone(),
                path: path.clone(),
                duration_ms: *duration_ms,
                sample_rate: *sample_rate,
                channels: *channels,
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

### 3. Frontend Search Integration

**Add to existing search interface:**

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
  let maxDuration = $state<number | undefined>(undefined);  // seconds
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
        durationMin: undefined,
        durationMax: maxDuration ? maxDuration * 1000 : undefined,
        pathFilter: undefined,
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
</script>

<!-- Search interface with optional duration filter -->
<input
  type="text"
  bind:value={searchQuery}
  placeholder="Search audio... (e.g., 'footsteps on wood')"
/>
<input
  type="number"
  bind:value={maxDuration}
  placeholder="Max duration (s)"
/>
<button onclick={handleSearch}>Search</button>

<!-- Display results with similarity scores -->
{#each results as result}
  <div>
    <span>{result.name}</span>
    <span>{(result.similarity * 100).toFixed(1)}% match</span>
  </div>
{/each}
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
- **Development**: Download manually and cache locally (e.g., `models/clap/` in project directory)
- **Production**: Store in app data directory: `$APPDATA/asseteer/models/clap/`
- Model acquisition strategy to be determined post-MVP

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

### 2. Embedding Search Optimization for Large Libraries (200K+ files)

**Primary Strategy: Filtered Linear Search**
```rust
// Pre-filter before loading embeddings
// Example: Duration filter reduces 200K files → 50K files → ~125-250ms search
let audio_data = sqlx::query_as(
    "SELECT ... WHERE duration_ms BETWEEN ? AND ? AND clap_embedding IS NOT NULL"
)
.bind(duration_min)
.bind(duration_max)
.fetch_all(pool).await?;

// Parallel cosine similarity with rayon
let results: Vec<_> = audio_data
    .par_iter()
    .map(|data| calculate_similarity(data))
    .collect();
```

**Performance Expectations:**
- **200K unfiltered**: ~500ms-1s (acceptable for broad search)
- **50K filtered**: ~125-250ms (typical use case)
- **10K filtered**: ~50-100ms (narrow search)

**Advanced Option (Post-MVP):**
- **Vector Database (usearch, qdrant)**: Sub-100ms for 200K+ files
- Only needed if filtered linear search is too slow in practice
- Adds complexity: index building, memory overhead (~100-200MB)

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

- [ ] Process 100-1000 audio files successfully
- [ ] Search queries return relevant results:
  - [ ] "footsteps on wood" - finds footstep sounds
  - [ ] "explosion sound" - finds explosions/impacts
  - [ ] "gentle wind" - finds ambient wind
  - [ ] "door closing" - finds door sounds
  - [ ] "metal impact" - finds metallic sounds
- [ ] Duration filtering reduces search space effectively
- [ ] Search completes in <1s for 200K files (unfiltered)
- [ ] Search completes in <250ms for 50K files (filtered)
- [ ] Error handling for corrupted/unsupported audio files
- [ ] Works with both short sound effects and long music tracks

---

## Implementation Phases

### Phase 0: Audio Processing Primitives (3-5 days)
**Goal:** Implement foundational audio processing components

- [ ] Implement STFT with rustfft (Hann window, configurable FFT size)
- [ ] Implement mel filterbank creation (Hz→mel conversion, triangular filters)
- [ ] Implement Symphonia audio loading (multi-format support)
- [ ] Implement audio resampling to 32kHz (rubato)
- [ ] Unit tests with sample audio files
- [ ] Verify mel spectrograms match expected shape

**Deliverable:** Working audio→mel spectrogram pipeline

### Phase 1: CLAP Integration (3-5 days)
**Goal:** Get CLAP embeddings working

- [ ] Add ONNX Runtime + tokenizers dependencies
- [ ] Implement `ClapModel` with audio/text encoders
- [ ] Load CLAP models and tokenizer from local directory
- [ ] Implement text encoding (tokenize → embed)
- [ ] Implement audio encoding (mel spec → embed)
- [ ] Unit tests verifying 512-dim embeddings

**Deliverable:** Functions for audio→embedding and text→embedding

### Phase 2: Processing Pipeline (3-5 days)
**Goal:** Process audio files and store embeddings

- [ ] Define database schema (audio_metadata with clap_embedding BLOB)
- [ ] Integrate CLAP into AudioProcessor
- [ ] Implement batch processing (existing task system)
- [ ] Store embeddings as BLOBs in SQLite
- [ ] Test with 100-1000 audio files
- [ ] Benchmark: 50-120ms per file

**Deliverable:** Audio files with CLAP embeddings in database

### Phase 3: Search Backend (2-3 days)
**Goal:** Text-to-audio search functionality

- [ ] Implement `search_audio_by_text` Tauri command
- [ ] Add duration/path filtering to pre-filter search space
- [ ] Parallel cosine similarity with rayon
- [ ] Sort by similarity, return top N results
- [ ] Integration tests with sample queries

**Deliverable:** Working search command returning ranked results

### Phase 4: Frontend Integration (2-3 days)
**Goal:** Add search to existing UI

- [ ] Integrate search into existing audio interface
- [ ] Add search query input + optional duration filter
- [ ] Display results with similarity scores
- [ ] Add loading states and error handling
- [ ] Test with various queries

**Deliverable:** Working audio search in application UI

**Total Estimated Time:** 2-4 weeks

---

## Success Metrics

### Performance Targets
- ✅ **Processing Speed**: 50-120ms per audio file
- ✅ **Search Latency**:
  - 200K files unfiltered: ~500ms-1s
  - 50K files filtered: ~125-250ms
  - 10K files filtered: ~50-100ms
- ✅ **VRAM Usage**: <1.5GB during processing
- ✅ **RAM Usage**: ~400MB for 200K embeddings

### Quality Targets
- ✅ **Search Precision**: >80% relevant results in top 10
- ✅ **Query Understanding**: Handles arbitrary text descriptions
- ✅ **Example Success**: Query "footsteps on wood" ranks footstep sounds in top 5
- ✅ **Handles Long-Form**: Works with both sound effects and music tracks

### User Experience
- ✅ **Search Response**: Results appear in <1 second (with reasonable filters)
- ✅ **Filtering**: Duration/path filters enable targeted search
- ✅ **Error Handling**: Clear error messages for failures

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

### Risk 3: Search Performance at 200K Scale
**Problem:** Linear scan of 200K embeddings may be too slow

**Mitigations:**
- Pre-filter by duration/path to reduce search space
- Parallel similarity calculation with rayon
- Profile with realistic 200K dataset
- If needed: Implement ANN search (usearch, qdrant) post-MVP
- Current target: <1s unfiltered, <250ms filtered (acceptable)

### Risk 4: Audio Processing Complexity
**Problem:** STFT and mel filterbank implementation may have edge cases

**Mitigations:**
- Unit tests with known audio samples
- Verify mel spectrogram dimensions match CLAP requirements
- Test with various audio formats and sample rates
- Reference implementations available in Python (librosa)

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
- Channel count (mono/stereo)
- File format filtering
- Folder hierarchy filtering

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

Implementing CLAP-based audio search provides **dramatic improvements** over filename-only search:

**Key Benefits:**
- ✅ **Content-based search**: Understands what audio *sounds like*, not just filename
- ✅ **Natural language queries**: "footsteps on wood floor", "gentle wind", "metal impact"
- ✅ **Zero-shot learning**: No predefined categories needed
- ✅ **Semantic understanding**: Knows "explosion" relates to "blast", "boom", "impact"
- ✅ **Scalable**: Handles 200K+ files with filtered search strategy

**Implementation Effort:**
- **2-4 weeks** for working implementation
- **Moderate complexity**: Audio processing (STFT, mel spectrograms) + ML inference (ONNX)
- **Well-documented**: CLAP models, ONNX Runtime, Symphonia all have good docs
- **Phase 0 critical**: Audio primitives (STFT, mel) must be implemented correctly

**Transformation:**
Current: Search for `"footstep.wav"` → finds only files named "footstep"
With CLAP: Search for `"footsteps on wood"` → finds all footstep sounds regardless of filename

This feature will **transform audio asset discoverability**, making it dramatically easier for users to find sounds based on what they hear in their head rather than guessing filenames.

---

## Plan Adjustments Summary

**Key changes from original plan:**

1. **Schema**: No migrations needed - fresh database schema defined directly
2. **Baseline**: Removed PANNs comparisons - upgrading from FTS5 filename search only
3. **Scale**: Updated for 200K file target with filtered search strategy
4. **T-CLAP**: Sticking with CLAP (not T-CLAP) for initial implementation
5. **Implementation**: Added Phase 0 for audio primitives (STFT, mel, loading)
6. **Tokenization**: Using `tokenizers` crate (solved, not a risk)
7. **Performance**: Realistic targets for 200K files (~500ms unfiltered, ~125-250ms filtered)
8. **Model hosting**: Manual download for development, production strategy TBD
9. **Dependencies**: Complete list including rustfft, tokenizers, ndarray, num-complex
10. **UI**: Simplified frontend examples - integrates with existing Tauri+Svelte UI
