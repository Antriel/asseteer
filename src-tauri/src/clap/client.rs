//! HTTP client for the CLAP embedding server

use reqwest::{multipart, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::OnceCell;

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

/// Async HTTP client for communicating with the CLAP Python server
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

    /// Check if the CLAP server is running and healthy
    pub async fn health_check(&self) -> Result<(), String> {
        let url = format!("{}/health", self.base_url);
        self.client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;
        Ok(())
    }

    /// Trigger model preloading on the server
    pub async fn preload(&self) -> Result<(), String> {
        let url = format!("{}/preload", self.base_url);
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
        let url = format!("{}/embed/text", self.base_url);
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
    pub async fn embed_audio_path(&self, path: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio", self.base_url);
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
    pub async fn embed_audio_bytes(&self, bytes: Vec<u8>, filename: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/embed/audio/upload", self.base_url);

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
