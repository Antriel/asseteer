use crate::{models::*, AppState};
use serde::Serialize;
use sqlx;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Progress event payload for scan operations
#[derive(Clone, Serialize)]
pub struct ScanProgress {
    pub phase: String,
    pub files_found: usize,
    pub files_inserted: usize,
    pub files_total: usize,
    pub zips_scanned: usize,
    pub current_path: Option<String>,
}

/// Supported image extensions
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Supported audio extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac"];

/// Represents a discovered asset (either a regular file or a zip entry)
#[derive(Debug)]
struct DiscoveredAsset {
    /// Filename (without path)
    filename: String,
    /// Path to the file (or zip file if this is a zip entry)
    path: PathBuf,
    /// If this asset is inside a zip, this is the path within the zip
    zip_entry: Option<String>,
    /// File extension
    format: String,
    /// Asset type (image or audio)
    asset_type: AssetType,
    /// File size in bytes
    file_size: i64,
}

/// Start a new scan session
#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    root_path: String,
) -> Result<i64, String> {
    let root_path_buf = PathBuf::from(&root_path);

    if !root_path_buf.exists() {
        return Err(format!("Path does not exist: {}", root_path));
    }

    // Create scan session
    let session_id = create_scan_session(&state, &root_path).await?;

    // Discover files with progress events
    let files = discover_files(&app, &root_path_buf)?;
    let total = files.len();

    // Update session with total count
    update_session_total(&state, session_id, total).await?;

    // Insert assets with progress events
    insert_pending_assets(&app, &state, session_id, files).await?;

    // Mark session as complete
    update_session_status(&state, session_id, "complete").await?;

    Ok(session_id)
}

/// Create a new scan session
async fn create_scan_session(state: &State<'_, AppState>, root_path: &str) -> Result<i64, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let result = sqlx::query(
        "INSERT INTO scan_sessions (root_path, started_at) VALUES (?1, ?2)"
    )
    .bind(root_path)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(result.last_insert_rowid())
}

/// Update session with total file count
async fn update_session_total(
    state: &State<'_, AppState>,
    session_id: i64,
    total: usize,
) -> Result<(), String> {
    sqlx::query("UPDATE scan_sessions SET total_files = ?1 WHERE id = ?2")
        .bind(total as i64)
        .bind(session_id)
        .execute(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Update session status
async fn update_session_status(
    state: &State<'_, AppState>,
    session_id: i64,
    status: &str,
) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    if status == "complete" {
        sqlx::query("UPDATE scan_sessions SET status = ?1, completed_at = ?2 WHERE id = ?3")
            .bind(status)
            .bind(now)
            .bind(session_id)
            .execute(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        sqlx::query("UPDATE scan_sessions SET status = ?1 WHERE id = ?2")
            .bind(status)
            .bind(session_id)
            .execute(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Discover all supported asset files in a directory (including zip entries)
fn discover_files(app: &AppHandle, root_path: &Path) -> Result<Vec<DiscoveredAsset>, String> {
    let mut assets = Vec::new();
    let mut zips_scanned = 0usize;
    let mut last_emit = Instant::now();
    const EMIT_INTERVAL_MS: u128 = 100;

    let emit_progress = |app: &AppHandle, assets: &[DiscoveredAsset], zips: usize, path: Option<&str>| {
        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                phase: "discovering".to_string(),
                files_found: assets.len(),
                files_inserted: 0,
                files_total: 0,
                zips_scanned: zips,
                current_path: path.map(String::from),
            },
        );
    };

    // Initial progress event
    emit_progress(app, &assets, 0, Some(&root_path.to_string_lossy()));

    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_file() {
            // Get filename to check for macOS metadata files
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

            // Skip macOS metadata files (._filename) and hidden files
            if filename.starts_with("._") || filename.starts_with('.') {
                continue;
            }

            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();

                // Check if it's a supported media file
                if let Some(asset_type) = detect_asset_type(path) {
                    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
                    let file_size = metadata.len() as i64;

                    assets.push(DiscoveredAsset {
                        filename: filename.to_string(),
                        path: path.to_path_buf(),
                        zip_entry: None,
                        format: ext_str.to_string(),
                        asset_type,
                        file_size,
                    });

                    // Emit progress periodically
                    if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
                        emit_progress(app, &assets, zips_scanned, Some(&path.to_string_lossy()));
                        last_emit = Instant::now();
                    }
                }
                // Check if it's a zip file
                else if ext_str == "zip" {
                    // Process zip entries
                    match discover_zip_entries(path) {
                        Ok(mut zip_assets) => {
                            assets.append(&mut zip_assets);
                            zips_scanned += 1;
                            // Always emit after processing a zip (can be slow)
                            emit_progress(app, &assets, zips_scanned, Some(&path.to_string_lossy()));
                            last_emit = Instant::now();
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to process zip file {}: {}", path.display(), e);
                            // Continue scanning even if one zip fails
                        }
                    }
                }
            }
        }
    }

    // Final progress event for discovery phase
    emit_progress(app, &assets, zips_scanned, None);

    Ok(assets)
}

/// Discover supported assets inside a zip file
fn discover_zip_entries(zip_path: &Path) -> Result<Vec<DiscoveredAsset>, String> {
    let file = File::open(zip_path)
        .map_err(|e| format!("Failed to open zip: {}", e))?;

    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip archive: {}", e))?;

    let mut assets = Vec::new();

    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        // Skip directories
        if entry.is_dir() {
            continue;
        }

        let entry_path = entry.name().to_string();

        // Extract filename and extension from the entry path
        let entry_path_buf = PathBuf::from(&entry_path);

        // Get the filename to check for macOS metadata files
        let filename = entry_path_buf
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        // Skip macOS metadata files (._filename) and hidden files
        if filename.starts_with("._") || filename.starts_with('.') {
            continue;
        }

        if let Some(ext) = entry_path_buf.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();

            if let Some(asset_type) = detect_asset_type_from_ext(&ext_str) {
                assets.push(DiscoveredAsset {
                    filename: filename.to_string(),
                    path: zip_path.to_path_buf(),
                    zip_entry: Some(entry_path),
                    format: ext_str.to_string(),
                    asset_type,
                    file_size: entry.size() as i64,
                });
            }
        }
    }

    Ok(assets)
}

/// Detect asset type from file extension
fn detect_asset_type(path: &Path) -> Option<AssetType> {
    path.extension().and_then(|ext| {
        let ext_str = ext.to_string_lossy().to_lowercase();
        detect_asset_type_from_ext(&ext_str)
    })
}

/// Detect asset type from extension string
fn detect_asset_type_from_ext(ext: &str) -> Option<AssetType> {
    if IMAGE_EXTENSIONS.contains(&ext) {
        Some(AssetType::Image)
    } else if AUDIO_EXTENSIONS.contains(&ext) {
        Some(AssetType::Audio)
    } else {
        None
    }
}

/// Insert discovered files as pending assets (no processing yet)
async fn insert_pending_assets(
    app: &AppHandle,
    state: &State<'_, AppState>,
    _session_id: i64,
    assets: Vec<DiscoveredAsset>,
) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let total = assets.len();
    let mut inserted = 0usize;
    let mut last_emit = Instant::now();
    const EMIT_INTERVAL_MS: u128 = 100;

    // Emit start of inserting phase
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            phase: "inserting".to_string(),
            files_found: total,
            files_inserted: 0,
            files_total: total,
            zips_scanned: 0,
            current_path: None,
        },
    );

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    for asset in assets {
        let path_str = asset.path.to_string_lossy().to_string();

        let _result = sqlx::query(
            "INSERT OR IGNORE INTO assets (
                filename, path, zip_entry, asset_type, format, file_size,
                created_at, modified_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        )
        .bind(&asset.filename)
        .bind(&path_str)
        .bind(&asset.zip_entry)
        .bind(asset.asset_type.as_str())
        .bind(&asset.format)
        .bind(asset.file_size)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        inserted += 1;

        // Emit progress periodically
        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let _ = app.emit(
                "scan-progress",
                ScanProgress {
                    phase: "inserting".to_string(),
                    files_found: total,
                    files_inserted: inserted,
                    files_total: total,
                    zips_scanned: 0,
                    current_path: Some(asset.filename.clone()),
                },
            );
            last_emit = Instant::now();
        }
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    // Final event
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            phase: "complete".to_string(),
            files_found: total,
            files_inserted: total,
            files_total: total,
            zips_scanned: 0,
            current_path: None,
        },
    );

    Ok(())
}
