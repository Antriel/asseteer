use crate::{models::*, AppState};
use rusqlite::params;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use walkdir::WalkDir;

/// Supported image extensions
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Supported audio extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac"];

/// Start a new scan session
#[tauri::command]
pub async fn start_scan(
    state: State<'_, AppState>,
    root_path: String,
) -> Result<i64, String> {
    let root_path_buf = PathBuf::from(&root_path);

    if !root_path_buf.exists() {
        return Err(format!("Path does not exist: {}", root_path));
    }

    // Create scan session
    let session_id = create_scan_session(&state, &root_path).await?;

    // Discover files
    let files = discover_files(&root_path_buf)?;
    let total = files.len();

    // Update session with total count
    update_session_total(&state, session_id, total).await?;

    // For MVP, we'll process files synchronously and insert them as pending
    // In Week 2, we'll add actual processing with thumbnails
    insert_pending_assets(&state, session_id, files).await?;

    // Mark session as complete
    update_session_status(&state, session_id, "complete").await?;

    Ok(session_id)
}

/// Create a new scan session
async fn create_scan_session(state: &State<'_, AppState>, root_path: &str) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO scan_sessions (root_path, started_at) VALUES (?1, ?2)",
        params![root_path, now],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    Ok(id)
}

/// Update session with total file count
async fn update_session_total(
    state: &State<'_, AppState>,
    session_id: i64,
    total: usize,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE scan_sessions SET total_files = ?1 WHERE id = ?2",
        params![total as i64, session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Update session status
async fn update_session_status(
    state: &State<'_, AppState>,
    session_id: i64,
    status: &str,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    if status == "complete" {
        conn.execute(
            "UPDATE scan_sessions SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![status, now, session_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE scan_sessions SET status = ?1 WHERE id = ?2",
            params![status, session_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Discover all supported asset files in a directory
fn discover_files(root_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();

                if IMAGE_EXTENSIONS.contains(&ext_str.as_str())
                    || AUDIO_EXTENSIONS.contains(&ext_str.as_str())
                {
                    files.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(files)
}

/// Detect asset type from file extension
fn detect_asset_type(path: &Path) -> Option<AssetType> {
    path.extension().and_then(|ext| {
        let ext_str = ext.to_string_lossy().to_lowercase();

        if IMAGE_EXTENSIONS.contains(&ext_str.as_str()) {
            Some(AssetType::Image)
        } else if AUDIO_EXTENSIONS.contains(&ext_str.as_str()) {
            Some(AssetType::Audio)
        } else {
            None
        }
    })
}

/// Insert discovered files as pending assets (no processing yet)
async fn insert_pending_assets(
    state: &State<'_, AppState>,
    _session_id: i64,
    files: Vec<PathBuf>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    for path in files {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let path_str = path.to_string_lossy().to_string();

        let format = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        let asset_type = detect_asset_type(&path)
            .ok_or_else(|| "Unknown asset type".to_string())?;

        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        let file_size = metadata.len() as i64;

        tx.execute(
            "INSERT INTO assets (
                filename, path, asset_type, format, file_size,
                created_at, modified_at, processing_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                filename,
                path_str,
                asset_type.as_str(),
                format,
                file_size,
                now,
                now,
                "pending"
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}
