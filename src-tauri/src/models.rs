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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcessingCategory {
    Image,
    Audio,
    Clap,
}

impl ProcessingCategory {
    pub fn as_str(&self) -> &str {
        match self {
            ProcessingCategory::Image => "image",
            ProcessingCategory::Audio => "audio",
            ProcessingCategory::Clap => "clap",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "image" => Ok(ProcessingCategory::Image),
            "audio" => Ok(ProcessingCategory::Audio),
            "clap" => Ok(ProcessingCategory::Clap),
            _ => Err(format!("Invalid processing category: {}", s)),
        }
    }

    pub fn to_asset_type(&self) -> Option<AssetType> {
        match self {
            ProcessingCategory::Image => Some(AssetType::Image),
            ProcessingCategory::Audio => Some(AssetType::Audio),
            ProcessingCategory::Clap => None, // CLAP is a processing step, not an asset type
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCount {
    pub images: usize,
    pub audio: usize,
    pub total: usize,
}

/// Processing error stored in database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProcessingError {
    pub id: i64,
    pub asset_id: i64,
    pub category: String,
    pub error_message: String,
    pub occurred_at: i64,
    pub retry_count: i32,
    pub resolved_at: Option<i64>,
}

/// Processing error with asset info for frontend display
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProcessingErrorDetail {
    pub id: i64,
    pub asset_id: i64,
    pub filename: String,
    pub path: String,
    pub error_message: String,
    pub occurred_at: i64,
    pub retry_count: i32,
}
