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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProcessingTask {
    pub id: i64,
    pub asset_id: i64,
    pub task_type: String,
    pub status: String,
    pub priority: i32,
    pub retry_count: i32,
    pub max_retries: i32,

    // Timestamps
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,

    // Error tracking
    pub error_message: Option<String>,

    // Progress
    pub progress_current: i32,
    pub progress_total: i32,

    // Task-specific data
    pub input_params: Option<String>,
    pub output_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Queued,
    Processing,
    Paused,
    Complete,
    Error,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Queued => "queued",
            TaskStatus::Processing => "processing",
            TaskStatus::Paused => "paused",
            TaskStatus::Complete => "complete",
            TaskStatus::Error => "error",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => TaskStatus::Pending,
            "queued" => TaskStatus::Queued,
            "processing" => TaskStatus::Processing,
            "paused" => TaskStatus::Paused,
            "complete" => TaskStatus::Complete,
            "error" => TaskStatus::Error,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Thumbnail,
    Metadata,
}

impl TaskType {
    pub fn as_str(&self) -> &str {
        match self {
            TaskType::Thumbnail => "thumbnail",
            TaskType::Metadata => "metadata",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "thumbnail" => Some(TaskType::Thumbnail),
            "metadata" => Some(TaskType::Metadata),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskProgress {
    pub task_id: i64,
    pub asset_id: i64,
    pub task_type: String,
    pub status: String,
    pub progress_current: i32,
    pub progress_total: i32,
    pub current_file: String,
}
