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

    // Image metadata
    pub width: Option<u32>,
    pub height: Option<u32>,

    // Audio metadata
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,

    // Timestamps
    pub created_at: i64,
    pub modified_at: i64,

    // Processing state
    pub processing_status: String,
    pub processing_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetData {
    pub filename: String,
    pub path: String,
    pub zip_entry: Option<String>,
    pub asset_type: String,
    pub format: String,
    pub file_size: u64,

    pub width: Option<u32>,
    pub height: Option<u32>,

    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,

    pub thumbnail_data: Vec<u8>,

    pub modified_at: i64,
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

#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub thumbnail_data: Vec<u8>,
    pub processing_status: String,
    pub processing_error: Option<String>,
}
