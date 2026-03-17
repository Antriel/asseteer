//! CLAP (Contrastive Language-Audio Pretraining) integration module
//!
//! Provides async HTTP client for the Python CLAP server and embedding utilities.

mod client;
mod embedding;
mod server;
pub mod uv;

pub use client::{get_clap_client, HealthInfo};
pub use embedding::{blob_to_embedding, cosine_similarity, embedding_to_blob};
pub use server::{ensure_server_running, stop_server};
