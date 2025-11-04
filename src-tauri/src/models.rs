use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Asset {
    pub id: i64,
    pub filename: String,
    pub path: String,
    pub zip_entry: Option<String>,
    pub asset_type: String,
    pub format: String,
    pub file_size: i64,

    // Image metadata (from image_metadata table)
    pub width: Option<i32>,
    pub height: Option<i32>,

    // Audio metadata (from audio_metadata table)
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,

    // Timestamps
    pub created_at: i64,
    pub modified_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ImageMetadata {
    pub asset_id: i64,
    pub width: i32,
    pub height: i32,
    pub thumbnail_data: Vec<u8>,
    pub processed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AudioMetadata {
    pub asset_id: i64,
    pub duration_ms: i64,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub processed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSession {
    pub id: i64,
    pub root_path: String,
    pub total_files: Option<i64>,
    pub processed_files: i64,
    pub status: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub session_id: i64,
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetType {
    Image,
    Audio,
}

impl AssetType {
    pub fn as_str(&self) -> &str {
        match self {
            AssetType::Image => "image",
            AssetType::Audio => "audio",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCount {
    pub images: usize,
    pub audio: usize,
    pub total: usize,
}
