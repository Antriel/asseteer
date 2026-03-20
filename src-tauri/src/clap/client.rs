//! HTTP client for the CLAP embedding server

use reqwest::{multipart, Client};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use tokio::sync::OnceCell;

/// The port the CLAP server is listening on. Updated by server.rs when a free port is found.
static ACTIVE_PORT: AtomicU16 = AtomicU16::new(5555);

pub fn set_active_port(port: u16) {
    ACTIVE_PORT.store(port, Ordering::Relaxed);
}

pub fn get_active_port() -> u16 {
    ACTIVE_PORT.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    pub status: String,
    pub model: String,
    pub device: String,
    pub embedding_dim: i32,
    /// The port the server is listening on (injected by Rust, not from server JSON)
    #[serde(skip_deserializing, default)]
    pub port: u16,
}

#[derive(Serialize)]
struct TextRequest {
    text: String,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct AudioPathRequest {
    audio_path: String,
}

#[derive(Serialize)]
struct BatchAudioPathRequest {
    audio_paths: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct BatchEmbeddingItem {
    #[allow(dead_code)]
    path: String,
    embedding: Option<Vec<f32>>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct BatchEmbeddingResponse {
    results: Vec<BatchEmbeddingItem>,
}

/// Async HTTP client for communicating with the CLAP Python server
pub struct ClapClient {
    client: Client,
}

impl ClapClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", get_active_port())
    }

    /// Check if the CLAP server is running and healthy.
    ///
    /// Validates the response is actually from our CLAP server (not some other
    /// service that happens to be listening on the same port) by parsing the JSON
    /// and checking the expected fields are present with the right values.
    pub async fn health_check(&self) -> Result<(), String> {
        let url = format!("{}/health", self.base_url());
        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Health check returned: {}", response.status()));
        }

        let info = response
            .json::<HealthInfo>()
            .await
            .map_err(|_| "Port is in use by a different service (not CLAP server)".to_string())?;

        if info.status != "ok" {
            return Err(format!("CLAP server reported unhealthy status: {}", info.status));
        }

        Ok(())
    }

    /// Get detailed health info including device (CPU/GPU)
    pub async fn health_check_detailed(&self) -> Result<HealthInfo, String> {
        let url = format!("{}/health", self.base_url());
        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("Health check returned: {}", response.status()));
        }
        let mut info = response
            .json::<HealthInfo>()
            .await
            .map_err(|e| format!("Failed to parse health response: {}", e))?;
        info.port = get_active_port();
        Ok(info)
    }

    /// Trigger model preloading on the server
    pub async fn preload(&self) -> Result<(), String> {
        let url = format!("{}/preload", self.base_url());
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| format!("Preload request failed: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("Preload failed: {}", response.status()));
        }
        Ok(())
    }

    /// Generate text embedding from a query string
    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/text", self.base_url());
        let request = TextRequest {
            text: text.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server error: {}", response.status()));
        }

        let embed: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(embed.embedding)
    }

    /// Generate audio embedding from a file path
    #[allow(dead_code)]
    pub async fn embed_audio_path(&self, path: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio", self.base_url());
        let request = AudioPathRequest {
            audio_path: path.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server error: {}", response.status()));
        }

        let embed: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(embed.embedding)
    }

    /// Generate audio embedding from raw bytes (for ZIP files)
    #[allow(dead_code)]
    pub async fn embed_audio_bytes(&self, bytes: Vec<u8>, filename: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio/upload", self.base_url());

        let part = multipart::Part::bytes(bytes).file_name(filename.to_string());
        let form = multipart::Form::new().part("audio", part);

        let response = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Server error: {}", response.status()));
        }

        let embed: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(embed.embedding)
    }

    /// Generate audio embeddings for multiple file paths in a single batched request
    pub async fn embed_audio_batch_paths(&self, paths: &[String]) -> Result<Vec<Result<Vec<f32>, String>>, String> {
        let url = format!("{}/embed/audio/batch", self.base_url());
        let request = BatchAudioPathRequest {
            audio_paths: paths.to_vec(),
        };

        let response = self
            .client
            .post(&url)
            .timeout(Duration::from_secs(120))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Batch request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Batch server error {}: {}", status, body));
        }

        let batch: BatchEmbeddingResponse = response
            .json()
            .await
            .map_err(|e| format!("Batch parse error: {}", e))?;

        Ok(batch
            .results
            .into_iter()
            .map(|item| {
                if let Some(emb) = item.embedding {
                    Ok(emb)
                } else {
                    Err(item.error.unwrap_or_else(|| "Unknown error".to_string()))
                }
            })
            .collect())
    }

    /// Generate audio embeddings for multiple byte buffers in a single batched request
    pub async fn embed_audio_batch_bytes(
        &self,
        items: Vec<(Vec<u8>, String)>, // (bytes, filename)
    ) -> Result<Vec<Result<Vec<f32>, String>>, String> {
        let url = format!("{}/embed/audio/batch/upload", self.base_url());

        let mut form = multipart::Form::new();
        for (bytes, filename) in items {
            let part = multipart::Part::bytes(bytes).file_name(filename);
            form = form.part("files", part);
        }

        let response = self
            .client
            .post(&url)
            .timeout(Duration::from_secs(120))
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Batch upload request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Batch upload server error {}: {}", status, body));
        }

        let batch: BatchEmbeddingResponse = response
            .json()
            .await
            .map_err(|e| format!("Batch upload parse error: {}", e))?;

        Ok(batch
            .results
            .into_iter()
            .map(|item| {
                if let Some(emb) = item.embedding {
                    Ok(emb)
                } else {
                    Err(item.error.unwrap_or_else(|| "Unknown error".to_string()))
                }
            })
            .collect())
    }
}

impl Default for ClapClient {
    fn default() -> Self {
        Self::new()
    }
}

// Async singleton instance
static CLAP_CLIENT: OnceCell<ClapClient> = OnceCell::const_new();

/// Get the global CLAP client instance
pub async fn get_clap_client() -> &'static ClapClient {
    CLAP_CLIENT.get_or_init(|| async { ClapClient::new() }).await
}
