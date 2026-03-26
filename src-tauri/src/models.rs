use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Asset {
    pub id: i64,
    pub filename: String,
    pub folder_id: i64,
    pub rel_path: String,
    pub zip_file: Option<String>,
    pub zip_entry: Option<String>,
    pub zip_compression: Option<String>,
    pub asset_type: String,
    pub format: String,
    pub file_size: i64,
    pub fs_modified_at: Option<i64>,

    // Timestamps
    pub created_at: i64,
    pub modified_at: i64,

    // Transient — populated by JOIN with source_folders, not a DB column
    #[sqlx(default)]
    pub folder_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SourceFolder {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub added_at: i64,
    pub last_scanned_at: Option<i64>,
    pub asset_count: i64,
    pub status: String,
    /// JSON-encoded Vec<String> of warnings from the last scan/rescan. NULL if none.
    #[sqlx(default)]
    pub scan_warnings: Option<String>,
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
    pub rel_path: String,
    pub zip_file: Option<String>,
    pub zip_entry: Option<String>,
    pub folder_path: String,
    pub error_message: String,
    pub occurred_at: i64,
    pub retry_count: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_category_from_str_valid() {
        assert_eq!(
            ProcessingCategory::from_str("image").unwrap(),
            ProcessingCategory::Image
        );
        assert_eq!(
            ProcessingCategory::from_str("audio").unwrap(),
            ProcessingCategory::Audio
        );
        assert_eq!(
            ProcessingCategory::from_str("clap").unwrap(),
            ProcessingCategory::Clap
        );
    }

    #[test]
    fn test_processing_category_from_str_case_insensitive() {
        assert_eq!(
            ProcessingCategory::from_str("IMAGE").unwrap(),
            ProcessingCategory::Image
        );
        assert_eq!(
            ProcessingCategory::from_str("Audio").unwrap(),
            ProcessingCategory::Audio
        );
    }

    #[test]
    fn test_processing_category_from_str_invalid() {
        assert!(ProcessingCategory::from_str("invalid").is_err());
        assert!(ProcessingCategory::from_str("").is_err());
    }

    #[test]
    fn test_processing_category_roundtrip() {
        for cat in [
            ProcessingCategory::Image,
            ProcessingCategory::Audio,
            ProcessingCategory::Clap,
        ] {
            let s = cat.as_str();
            let restored = ProcessingCategory::from_str(s).unwrap();
            assert_eq!(cat, restored);
        }
    }

    #[test]
    fn test_processing_category_to_asset_type() {
        assert!(matches!(
            ProcessingCategory::Image.to_asset_type(),
            Some(AssetType::Image)
        ));
        assert!(matches!(
            ProcessingCategory::Audio.to_asset_type(),
            Some(AssetType::Audio)
        ));
        assert!(ProcessingCategory::Clap.to_asset_type().is_none());
    }

    #[test]
    fn test_asset_type_as_str() {
        assert_eq!(AssetType::Image.as_str(), "image");
        assert_eq!(AssetType::Audio.as_str(), "audio");
    }
}
