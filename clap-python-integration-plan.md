# CLAP Integration Plan: Python HTTP Server Approach

## Overview

Use Python for CLAP embedding generation via HTTP server, Rust for everything else. Server starts lazily when first needed.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Asset Processing Pipeline                    │
│                                                                   │
│  Audio File → Python CLAP Server → 512-dim embedding → SQLite   │
│               (lazy start, stays running)                        │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                       Search Pipeline                            │
│                                                                   │
│  User Query → Python CLAP → 512-dim embedding →                 │
│               Rust similarity search → Results                   │
│               (falls back to FTS if server unavailable)          │
└─────────────────────────────────────────────────────────────────┘
```

## Key Decisions

| Decision | Choice |
|----------|--------|
| Python framework | FastAPI (async, better performance) |
| Server lifecycle | Lazy auto-start when first needed |
| Server port | 5555 (localhost only) |
| ZIP file audio | Send raw bytes via `/embed/audio/upload` |
| CLAP processing | Integrated with audio processing + separate reprocess option |
| Search fallback | FTS when CLAP unavailable + show error/instructions |
| Production bundling | PyInstaller (future) |

---

## Database Schema

```sql
-- New table for CLAP embeddings
CREATE TABLE audio_embeddings (
    asset_id INTEGER PRIMARY KEY,
    embedding BLOB NOT NULL,  -- 512 floats = 2048 bytes
    model_version TEXT NOT NULL DEFAULT 'laion/clap-htsat-fused',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_audio_embeddings_model ON audio_embeddings(model_version);
```

---

## Python Server (FastAPI)

### File: `clap-python-prototype/clap_server.py`

Convert existing Flask server to FastAPI:

```python
#!/usr/bin/env python3
"""
CLAP HTTP Server (FastAPI)

Endpoints:
    POST /embed/text         - Generate text embedding
    POST /embed/audio        - Generate audio embedding from file path
    POST /embed/audio/upload - Generate audio embedding from raw bytes
    GET  /health             - Health check
"""

import io
import logging
from pathlib import Path
from contextlib import asynccontextmanager

import librosa
import numpy as np
from fastapi import FastAPI, HTTPException, UploadFile, File
from pydantic import BaseModel

from clap_test import ClapTester

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Global model instance
clap_model: ClapTester | None = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Load model on startup"""
    global clap_model
    logger.info("Loading CLAP model...")
    clap_model = ClapTester(model_name="laion/clap-htsat-fused")
    logger.info(f"Model loaded on {clap_model.device}")
    yield
    logger.info("Shutting down...")


app = FastAPI(title="CLAP Embedding Server", lifespan=lifespan)


class TextRequest(BaseModel):
    text: str


class AudioPathRequest(BaseModel):
    audio_path: str


class EmbeddingResponse(BaseModel):
    embedding: list[float]


class HealthResponse(BaseModel):
    status: str
    model: str
    device: str
    embedding_dim: int


@app.get("/health", response_model=HealthResponse)
async def health():
    return HealthResponse(
        status="ok",
        model="laion/clap-htsat-fused",
        device=clap_model.device if clap_model else "unknown",
        embedding_dim=512
    )


@app.post("/embed/text", response_model=EmbeddingResponse)
async def embed_text(request: TextRequest):
    if not request.text.strip():
        raise HTTPException(400, "Text cannot be empty")

    embedding = clap_model.encode_text(request.text)
    return EmbeddingResponse(embedding=embedding.tolist())


@app.post("/embed/audio", response_model=EmbeddingResponse)
async def embed_audio(request: AudioPathRequest):
    path = Path(request.audio_path)
    if not path.exists():
        raise HTTPException(404, f"File not found: {request.audio_path}")

    audio = clap_model.load_audio(str(path))
    embedding = clap_model.encode_audio(audio)
    return EmbeddingResponse(embedding=embedding.tolist())


@app.post("/embed/audio/upload", response_model=EmbeddingResponse)
async def embed_audio_upload(audio: UploadFile = File(...)):
    """Generate embedding from raw audio bytes (for ZIP files)"""
    content = await audio.read()

    target_sr = clap_model.processor.feature_extractor.sampling_rate
    audio_data, _ = librosa.load(io.BytesIO(content), sr=target_sr, mono=True)

    embedding = clap_model.encode_audio(audio_data)
    return EmbeddingResponse(embedding=embedding.tolist())


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=5555)
```

### requirements.txt changes

Replace Flask with FastAPI:
```diff
- flask>=3.0.0
+ fastapi>=0.109.0
+ uvicorn[standard]>=0.27.0
+ python-multipart>=0.0.6  # For file uploads
```

---

## Rust Implementation

### Add dependency to Cargo.toml

```toml
# HTTP client for CLAP server
reqwest = { version = "0.12", features = ["json", "multipart", "blocking"] }
```

### File: `src-tauri/src/clap/mod.rs`

```rust
mod client;
mod server;

pub use client::ClapClient;
pub use server::ensure_server_running;
```

### File: `src-tauri/src/clap/client.rs`

```rust
use reqwest::blocking::{Client, multipart};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

const CLAP_SERVER_URL: &str = "http://127.0.0.1:5555";

#[derive(Serialize)]
struct TextRequest {
    text: String,
}

#[derive(Serialize)]
struct AudioPathRequest {
    audio_path: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

pub struct ClapClient {
    client: Client,
    base_url: String,
}

impl ClapClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: CLAP_SERVER_URL.to_string(),
        }
    }

    pub fn health_check(&self) -> Result<(), String> {
        let url = format!("{}/health", self.base_url);
        self.client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .map_err(|e| format!("Health check failed: {}", e))?;
        Ok(())
    }

    pub fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/text", self.base_url);
        let request = TextRequest { text: text.to_string() };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server error: {}", response.status()));
        }

        let embed: EmbeddingResponse = response
            .json()
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(embed.embedding)
    }

    pub fn embed_audio_path(&self, path: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio", self.base_url);
        let request = AudioPathRequest { audio_path: path.to_string() };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server error: {}", response.status()));
        }

        let embed: EmbeddingResponse = response
            .json()
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(embed.embedding)
    }

    /// For audio files inside ZIP archives - send raw bytes
    pub fn embed_audio_bytes(&self, bytes: &[u8], filename: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio/upload", self.base_url);

        let part = multipart::Part::bytes(bytes.to_vec())
            .file_name(filename.to_string());
        let form = multipart::Form::new().part("audio", part);

        let response = self.client
            .post(&url)
            .multipart(form)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server error: {}", response.status()));
        }

        let embed: EmbeddingResponse = response
            .json()
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(embed.embedding)
    }
}

// Singleton
static CLAP_CLIENT: OnceLock<ClapClient> = OnceLock::new();

pub fn get_clap_client() -> &'static ClapClient {
    CLAP_CLIENT.get_or_init(ClapClient::new)
}
```

### File: `src-tauri/src/clap/server.rs`

```rust
use std::process::{Command, Stdio, Child};
use std::sync::Mutex;
use std::time::Duration;
use std::thread;

use super::client::get_clap_client;

static SERVER_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

/// Ensures CLAP server is running, starts it if needed
pub fn ensure_server_running() -> Result<(), String> {
    // Check if already running
    if get_clap_client().health_check().is_ok() {
        return Ok(());
    }

    // Try to start server
    let mut guard = SERVER_PROCESS.lock().map_err(|e| e.to_string())?;

    // Double-check after acquiring lock
    if get_clap_client().health_check().is_ok() {
        return Ok(());
    }

    // Start Python server
    let child = Command::new("python")
        .args(["-m", "uvicorn", "clap_server:app", "--host", "127.0.0.1", "--port", "5555"])
        .current_dir("clap-python-prototype")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start CLAP server: {}", e))?;

    *guard = Some(child);

    // Wait for server to be ready (max 30 seconds for model loading)
    for _ in 0..60 {
        thread::sleep(Duration::from_millis(500));
        if get_clap_client().health_check().is_ok() {
            return Ok(());
        }
    }

    Err("CLAP server failed to start within 30 seconds".to_string())
}

/// Stops the CLAP server if we started it
pub fn stop_server() {
    if let Ok(mut guard) = SERVER_PROCESS.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
}
```

---

## Embedding Storage Utilities

### File: `src-tauri/src/clap/embedding.rs`

```rust
/// Convert embedding Vec<f32> to BLOB bytes
pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// Convert BLOB bytes back to Vec<f32>
pub fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Cosine similarity (embeddings are L2-normalized)
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
```

---

## Processing Pipeline Integration

Integrate CLAP into existing `process_audio()` in `src-tauri/src/task_system/processor.rs`.

The `clap_enabled` flag is passed from frontend when starting processing - it also determines whether audio files without embeddings are considered "pending".

```rust
// In process_audio function, after extracting metadata:

// Generate CLAP embedding if enabled (flag passed from frontend)
if clap_enabled {
    match clap::ensure_server_running() {
        Ok(()) => {
            let embedding = if let Some(zip_path) = &asset.zip_entry {
                // Audio inside ZIP - send raw bytes
                let bytes = load_asset_bytes_from_zip(&asset.path, zip_path)?;
                clap::get_clap_client().embed_audio_bytes(&bytes, &asset.filename)?
            } else {
                // Regular file - send path
                clap::get_clap_client().embed_audio_path(&asset.path)?
            };

            // Store embedding
            let blob = clap::embedding_to_blob(&embedding);
            sqlx::query(
                "INSERT INTO audio_embeddings (asset_id, embedding, created_at)
                 VALUES (?, ?, ?)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     embedding = excluded.embedding,
                     created_at = excluded.created_at"
            )
            .bind(asset.id)
            .bind(&blob)
            .bind(chrono::Utc::now().timestamp())
            .execute(&pool)
            .await?;
        }
        Err(e) => {
            // Log but don't fail - CLAP is optional
            log::warn!("CLAP embedding skipped: {}", e);
        }
    }
}
```

---

## Search Implementation

### Tauri Command: `src-tauri/src/commands/search.rs`

```rust
use crate::clap::{self, blob_to_embedding, cosine_similarity};

#[derive(Serialize)]
pub struct SemanticSearchResult {
    pub asset_id: i64,
    pub filename: String,
    pub path: String,
    pub similarity: f32,
}

#[tauri::command]
pub async fn search_audio_semantic(
    query: String,
    limit: usize,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<SemanticSearchResult>, String> {
    // Ensure server is running
    clap::ensure_server_running()?;

    // Get query embedding
    let query_embedding = clap::get_clap_client().embed_text(&query)?;

    // Fetch all embeddings
    let rows = sqlx::query!(
        r#"
        SELECT a.id, a.filename, a.path, ae.embedding
        FROM assets a
        JOIN audio_embeddings ae ON a.id = ae.asset_id
        "#
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Compute similarities
    let mut results: Vec<_> = rows.iter().map(|row| {
        let embedding = blob_to_embedding(&row.embedding);
        let similarity = cosine_similarity(&query_embedding, &embedding);
        SemanticSearchResult {
            asset_id: row.id,
            filename: row.filename.clone(),
            path: row.path.clone(),
            similarity,
        }
    }).collect();

    // Sort by similarity descending
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    results.truncate(limit);

    Ok(results)
}
```

---

## Frontend Integration

### Search with fallback: `src/lib/database/queries.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';

export interface SemanticSearchResult {
  asset_id: number;
  filename: string;
  path: string;
  similarity: number;
}

export async function searchAudioSemantic(
  query: string,
  limit: number = 50
): Promise<SemanticSearchResult[]> {
  try {
    return await invoke('search_audio_semantic', { query, limit });
  } catch (error) {
    // Show error to user, fall back to FTS
    console.error('Semantic search failed:', error);
    throw error; // Let UI handle fallback
  }
}
```

---

## Implementation Checklist

### Phase 1: Python Server (FastAPI)
- [ ] Convert `clap_server.py` from Flask to FastAPI
- [ ] Update `requirements.txt` with FastAPI dependencies
- [ ] Test all endpoints work correctly

### Phase 2: Database
- [ ] Add `audio_embeddings` table to schema.rs
- [ ] Run schema update on app start

### Phase 3: Rust Client
- [ ] Add `reqwest` to Cargo.toml
- [ ] Create `src-tauri/src/clap/` module
- [ ] Implement `client.rs` (HTTP client)
- [ ] Implement `server.rs` (lazy start)
- [ ] Implement `embedding.rs` (blob conversion)

### Phase 4: Processing Integration
- [ ] Accept `clap_enabled` flag in `start_processing` command
- [ ] Integrate embedding generation into `process_audio()`
- [ ] Handle ZIP file audio via raw bytes
- [ ] Query for "pending CLAP" assets (audio without embeddings)
- [ ] Add "reprocess for CLAP" command

### Phase 5: Search
- [ ] Implement `search_audio_semantic` command
- [ ] Add frontend search function with fallback
- [ ] UI for semantic search (separate from FTS, or combined - TBD)

### Phase 6: Production (Future)
- [ ] Bundle Python server with PyInstaller
- [ ] Auto-extract and run bundled server
- [ ] Handle server lifecycle on app exit

---

## Performance Expectations

| Operation | Expected Time |
|-----------|---------------|
| Text embedding | 20-50ms |
| Audio embedding | 50-150ms |
| Similarity search (10k files) | 1-5ms |
| Server cold start | 10-30s (model loading) |

---

## Error Handling

1. **Server not running**:
   - Try lazy start
   - If fails, show instructions: "Run `python clap_server.py` in clap-python-prototype/"
   - Fall back to FTS for search

2. **Embedding fails**:
   - Log error, continue processing other files
   - Don't mark audio as failed (metadata still valid)

3. **Search fails**:
   - Show toast with error
   - Automatically use FTS results instead
