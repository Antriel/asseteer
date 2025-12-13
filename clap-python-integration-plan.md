# CLAP Integration Plan: Python-Based Approach

## Overview

Use Python for CLAP embedding generation, Rust for everything else. This avoids implementing HTSAT in Rust while maintaining performance.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Asset Processing Pipeline                    │
│                                                                   │
│  Audio File → Python CLAP → 512-dim embedding → SQLite DB       │
│               (offline, one-time)                                │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                       Search Pipeline                            │
│                                                                   │
│  User Query → Python CLAP → 512-dim embedding →                 │
│               Rust similarity search (SQL) → Results             │
│               (real-time, <50ms)                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Database Schema Changes

```sql
-- Add embedding column to audio_metadata table
ALTER TABLE audio_metadata ADD COLUMN clap_embedding BLOB;

-- Or create separate table for better organization
CREATE TABLE audio_embeddings (
    asset_id INTEGER PRIMARY KEY,
    embedding BLOB NOT NULL,  -- 512 floats (2048 bytes)
    model_version TEXT NOT NULL DEFAULT 'laion/clap-htsat-fused',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_audio_embeddings_model ON audio_embeddings(model_version);
```

## Implementation Options

### Option A: Subprocess (Simplest)

**How it works:**
- Spawn Python process for each embedding request
- Pass data via command-line arguments or stdin
- Parse JSON output

**Implementation:**

```rust
// src-tauri/src/clap/subprocess.rs

use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbedRequest {
    text: Option<String>,
    audio_path: Option<String>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

pub fn embed_text(query: &str) -> Result<Vec<f32>, String> {
    let output = Command::new("python")
        .arg("clap-python-prototype/embed_text.py")
        .arg(query)
        .output()
        .map_err(|e| format!("Failed to execute Python: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let response: EmbedResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(response.embedding)
}

pub fn embed_audio(audio_path: &str) -> Result<Vec<f32>, String> {
    let output = Command::new("python")
        .arg("clap-python-prototype/embed_audio.py")
        .arg(audio_path)
        .output()
        .map_err(|e| format!("Failed to execute Python: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let response: EmbedResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(response.embedding)
}
```

**Python scripts:**

```python
# clap-python-prototype/embed_text.py
import sys
import json
from clap_test import ClapTester

def main():
    text = sys.argv[1]

    tester = ClapTester()
    embedding = tester.encode_text(text)

    print(json.dumps({"embedding": embedding.tolist()}))

if __name__ == "__main__":
    main()
```

**Pros:**
- ✅ Simplest implementation (~1 hour)
- ✅ No dependencies
- ✅ Easy to debug

**Cons:**
- ❌ Slow (~100-200ms per query due to Python startup)
- ❌ Model loads for every request
- ❌ Not suitable for real-time queries

**Best for:** Offline audio processing only

---

### Option B: HTTP Server (Recommended)

**How it works:**
- Start Python HTTP server once (long-running)
- Model loads once and stays in memory
- Rust makes HTTP requests for embeddings

**Implementation:**

```python
# clap-python-prototype/clap_server.py
from flask import Flask, request, jsonify
from clap_test import ClapTester
import numpy as np

app = Flask(__name__)

# Load model once at startup
print("Loading CLAP model...")
model = ClapTester()
print("Model loaded!")

@app.route("/embed/text", methods=["POST"])
def embed_text():
    """Generate text embedding"""
    data = request.json
    text = data.get("text")

    if not text:
        return jsonify({"error": "Missing 'text' field"}), 400

    embedding = model.encode_text(text)
    return jsonify({"embedding": embedding.tolist()})

@app.route("/embed/audio", methods=["POST"])
def embed_audio():
    """Generate audio embedding from file path"""
    data = request.json
    audio_path = data.get("audio_path")

    if not audio_path:
        return jsonify({"error": "Missing 'audio_path' field"}), 400

    audio = model.load_audio(audio_path)
    embedding = model.encode_audio(audio)
    return jsonify({"embedding": embedding.tolist()})

@app.route("/health", methods=["GET"])
def health():
    """Health check endpoint"""
    return jsonify({"status": "ok", "model": "laion/clap-htsat-fused"})

if __name__ == "__main__":
    app.run(host="127.0.0.1", port=5555, debug=False)
```

```rust
// src-tauri/src/clap/http_client.rs

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct ClapClient {
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct TextRequest {
    text: String,
}

#[derive(Serialize)]
struct AudioRequest {
    audio_path: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

impl ClapClient {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    pub fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/text", self.base_url);
        let request = TextRequest {
            text: text.to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!("Server error {}: {}", status, body));
        }

        let embed_response: EmbedResponse = response
            .json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(embed_response.embedding)
    }

    pub fn embed_audio(&self, audio_path: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio", self.base_url);
        let request = AudioRequest {
            audio_path: audio_path.to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!("Server error {}: {}", status, body));
        }

        let embed_response: EmbedResponse = response
            .json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(embed_response.embedding)
    }

    pub fn health_check(&self) -> Result<(), String> {
        let url = format!("{}/health", self.base_url);

        self.client
            .get(&url)
            .send()
            .map_err(|e| format!("Health check failed: {}", e))?;

        Ok(())
    }
}

// Singleton instance
use std::sync::OnceLock;

static CLAP_CLIENT: OnceLock<ClapClient> = OnceLock::new();

pub fn get_clap_client() -> &'static ClapClient {
    CLAP_CLIENT.get_or_init(|| {
        ClapClient::new("http://127.0.0.1:5555")
    })
}
```

**Starting the server:**

```rust
// src-tauri/src/main.rs or lib.rs

fn start_clap_server() -> Result<(), String> {
    use std::process::{Command, Stdio};

    // Start Python server in background
    let _child = Command::new("python")
        .arg("clap-python-prototype/clap_server.py")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start CLAP server: {}", e))?;

    // Wait for server to be ready
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Health check
    let client = get_clap_client();
    client.health_check()?;

    Ok(())
}
```

**Pros:**
- ✅ Fast (~20-50ms per query)
- ✅ Model loaded once
- ✅ Simple HTTP protocol
- ✅ Easy to test independently
- ✅ Can scale to multiple workers if needed

**Cons:**
- ⚠️ Need to manage Python process lifecycle
- ⚠️ Extra dependency (Flask/FastAPI)
- ⚠️ Network overhead (minimal for localhost)

**Best for:** Production use, real-time queries

---

### Option C: PyO3 (Best Performance)

**How it works:**
- Embed Python interpreter in Rust process
- Call Python functions directly via C API
- No IPC overhead

**Implementation:**

```toml
# Cargo.toml
[dependencies]
pyo3 = { version = "0.20", features = ["auto-initialize"] }
```

```rust
// src-tauri/src/clap/pyo3_bridge.rs

use pyo3::prelude::*;
use pyo3::types::{PyModule, PyList};

pub struct ClapModel {
    py_model: Py<PyAny>,
}

impl ClapModel {
    pub fn new() -> Result<Self, String> {
        Python::with_gil(|py| {
            // Add Python script directory to path
            let sys = py.import("sys")?;
            let path: &PyList = sys.getattr("path")?.downcast()?;
            path.insert(0, "clap-python-prototype")?;

            // Import module
            let clap_module = PyModule::import(py, "clap_test")?;

            // Create model instance
            let model_class = clap_module.getattr("ClapTester")?;
            let model = model_class.call0()?;

            Ok(Self {
                py_model: model.into(),
            })
        }).map_err(|e: PyErr| format!("Failed to initialize CLAP model: {}", e))
    }

    pub fn encode_text(&self, text: &str) -> Result<Vec<f32>, String> {
        Python::with_gil(|py| {
            let result = self.py_model
                .call_method1(py, "encode_text", (text,))?;

            // Convert numpy array to Vec<f32>
            let embedding: Vec<f32> = result
                .call_method0(py, "tolist")?
                .extract(py)?;

            Ok(embedding)
        }).map_err(|e: PyErr| format!("Failed to encode text: {}", e))
    }

    pub fn encode_audio(&self, audio_path: &str) -> Result<Vec<f32>, String> {
        Python::with_gil(|py| {
            // Load audio
            let audio = self.py_model
                .call_method1(py, "load_audio", (audio_path,))?;

            // Encode
            let result = self.py_model
                .call_method1(py, "encode_audio", (audio,))?;

            // Convert to Vec<f32>
            let embedding: Vec<f32> = result
                .call_method0(py, "tolist")?
                .extract(py)?;

            Ok(embedding)
        }).map_err(|e: PyErr| format!("Failed to encode audio: {}", e))
    }
}

// Singleton instance
use std::sync::OnceLock;

static CLAP_MODEL: OnceLock<ClapModel> = OnceLock::new();

pub fn get_clap_model() -> &'static ClapModel {
    CLAP_MODEL.get_or_init(|| {
        ClapModel::new().expect("Failed to initialize CLAP model")
    })
}
```

**Pros:**
- ✅ Fastest (~10-20ms per query)
- ✅ No IPC overhead
- ✅ Single process
- ✅ Model loaded once

**Cons:**
- ❌ Complex setup (Python must be embedded correctly)
- ❌ Platform-specific issues
- ❌ Harder to debug
- ❌ Version compatibility issues (Python, PyTorch, etc.)

**Best for:** Maximum performance, if HTTP server isn't fast enough

---

## Similarity Search Implementation

```rust
// src-tauri/src/commands/search.rs

use crate::clap::http_client::get_clap_client;

#[derive(Serialize)]
pub struct AudioSearchResult {
    pub asset_id: i64,
    pub file_name: String,
    pub file_path: String,
    pub similarity: f32,
}

pub fn search_audio_by_text(
    pool: &SqlitePool,
    query: &str,
    limit: usize,
) -> Result<Vec<AudioSearchResult>, String> {
    // 1. Get query embedding from Python
    let client = get_clap_client();
    let query_embedding = client.embed_text(query)?;

    // 2. Search database with cosine similarity
    let results = sqlx::query_as!(
        AudioSearchResult,
        r#"
        SELECT
            a.id as asset_id,
            a.file_name,
            a.file_path,
            -- Cosine similarity (embeddings are already L2-normalized)
            (
                SELECT SUM(q.value * e.value)
                FROM json_each($1) q
                JOIN json_each(ae.embedding) e ON q.key = e.key
            ) as similarity
        FROM assets a
        JOIN audio_embeddings ae ON a.id = ae.asset_id
        WHERE a.asset_type = 'audio'
        ORDER BY similarity DESC
        LIMIT $2
        "#,
        serde_json::to_string(&query_embedding).unwrap(),
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(results)
}
```

**Better: Use BLOB directly (faster):**

```rust
// Store embeddings as raw f32 bytes
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

// Compute similarity in Rust (much faster)
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn search_audio_by_text(
    pool: &SqlitePool,
    query: &str,
    limit: usize,
) -> Result<Vec<AudioSearchResult>, String> {
    // 1. Get query embedding
    let client = get_clap_client();
    let query_embedding = client.embed_text(query)?;

    // 2. Fetch all embeddings (or use pagination for huge datasets)
    let rows = sqlx::query!(
        r#"
        SELECT a.id, a.file_name, a.file_path, ae.embedding
        FROM assets a
        JOIN audio_embeddings ae ON a.id = ae.asset_id
        WHERE a.asset_type = 'audio'
        "#
    )
    .fetch_all(pool)
    .await?;

    // 3. Compute similarities in Rust
    let mut results: Vec<_> = rows.iter().map(|row| {
        let embedding = blob_to_embedding(&row.embedding);
        let similarity = cosine_similarity(&query_embedding, &embedding);

        AudioSearchResult {
            asset_id: row.id,
            file_name: row.file_name.clone(),
            file_path: row.file_path.clone(),
            similarity,
        }
    }).collect();

    // 4. Sort and limit
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    results.truncate(limit);

    Ok(results)
}
```

---

## Processing Pipeline Integration

```rust
// src-tauri/src/commands/process.rs

async fn process_audio_batch(
    pool: &SqlitePool,
    assets: Vec<Asset>,
) -> Result<(), String> {
    let client = get_clap_client();

    for asset in assets {
        // Generate embedding
        let embedding = client.embed_audio(&asset.file_path)?;

        // Store in database
        let blob = embedding_to_blob(&embedding);

        sqlx::query!(
            r#"
            INSERT INTO audio_embeddings (asset_id, embedding, created_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (asset_id) DO UPDATE SET
                embedding = excluded.embedding,
                created_at = excluded.created_at
            "#,
            asset.id,
            blob,
            chrono::Utc::now().timestamp()
        )
        .execute(pool)
        .await?;

        // Mark as processed
        sqlx::query!(
            "UPDATE assets SET processing_status = 'complete' WHERE id = $1",
            asset.id
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}
```

---

## Deployment

### Development Mode
```bash
# Terminal 1: Start Python server
cd clap-python-prototype
python clap_server.py

# Terminal 2: Run Tauri app
npm run tauri dev
```

### Production Mode

**Option 1: Bundle Python with app**
- Use PyInstaller to bundle Python + dependencies
- Ship as separate executable
- Start automatically from Tauri

**Option 2: Require Python installation**
- Document Python requirements
- Check for Python at startup
- Show error if not found

**Option 3: Use PyO3 (embedded)**
- Python runtime embedded in binary
- No external dependencies
- Larger binary size

---

## Performance Estimates

| Operation | Option A (Subprocess) | Option B (HTTP) | Option C (PyO3) |
|-----------|----------------------|-----------------|-----------------|
| Text embedding | 100-200ms | 20-50ms | 10-20ms |
| Audio embedding | 150-300ms | 50-100ms | 30-60ms |
| Similarity search (10k files) | 1-5ms | 1-5ms | 1-5ms |
| **Total query time** | **100-210ms** | **25-60ms** | **15-30ms** |

---

## Recommended Implementation Plan

### Phase 1: HTTP Server Approach (3-4 hours)

1. **Create Python HTTP server** (1 hour)
   - `clap_server.py` with Flask
   - `/embed/text` and `/embed/audio` endpoints
   - Health check endpoint

2. **Update database schema** (30 min)
   - Add `audio_embeddings` table
   - No migration script necessary

3. **Implement Rust HTTP client** (1 hour)
   - `clap/http_client.rs`
   - Connection pooling
   - Error handling

4. **Integrate with processing pipeline** (1 hour)
   - Call Python for audio embeddings
   - Store in database

5. **Implement similarity search** (30 min)
   - Search command
   - Frontend integration

### Phase 2: Optimization (if needed)

- Switch to PyO3 if HTTP is too slow
- Add caching layer
- Batch embedding requests

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clap_text_embedding() {
        let client = get_clap_client();
        let embedding = client.embed_text("footsteps on wood").unwrap();

        assert_eq!(embedding.len(), 512);

        // Check normalized
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_search() {
        // Create test database
        // Insert test embeddings
        // Run search
        // Verify results ordered by similarity
    }
}
```

---

## Future Enhancements

1. **Caching**: Cache text embeddings for common queries
2. **Batch processing**: Process multiple audio files in parallel
3. **Vector database**: Use specialized DB (Qdrant, Milvus) for huge datasets
4. **Hybrid search**: Combine CLAP with FTS5 for better results
5. **Fine-tuning**: Train CLAP on your specific audio types

---

## Decision Matrix

| Criteria | Subprocess | HTTP Server | PyO3 |
|----------|-----------|-------------|------|
| **Implementation time** | 1 hour | 3 hours | 6 hours |
| **Query performance** | Poor (100ms+) | Good (20-50ms) | Excellent (10-20ms) |
| **Ease of debugging** | Easy | Easy | Hard |
| **Production readiness** | Low | High | Medium |
| **Deployment complexity** | Low | Medium | High |
| **Recommended?** | No | **Yes** | Only if needed |

---

## Conclusion

**Use HTTP Server approach (Option B)** for:
- ✅ Fast enough for real-time search (20-50ms)
- ✅ Simple implementation (3-4 hours)
- ✅ Easy to test and debug
- ✅ Production-ready
- ✅ Reuses working Python code

Start with this, measure performance, and only switch to PyO3 if needed.
