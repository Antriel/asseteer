use crate::{models::*, AppState};
use serde::Serialize;
use sqlx;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
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

/// Number of assets per chunk sent through the channel
const CHUNK_SIZE: usize = 200;

/// Minimum interval between progress event emissions
const EMIT_INTERVAL_MS: u128 = 100;

/// Represents a discovered asset (either a regular file or a zip entry)
#[derive(Debug)]
struct DiscoveredAsset {
    filename: String,
    path: PathBuf,
    zip_entry: Option<String>,
    format: String,
    asset_type: AssetType,
    file_size: i64,
}

/// Shared progress counters between discovery and insertion tasks
struct ScanProgressState {
    files_found: AtomicUsize,
    files_inserted: AtomicUsize,
    zips_scanned: AtomicUsize,
    discovery_complete: AtomicBool,
}

/// Start a new scan session
///
/// Discovery runs on a blocking thread and streams asset chunks through a channel.
/// Insertion runs concurrently on the async runtime, inserting chunks as they arrive.
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

    let session_id = create_scan_session(&state, &root_path).await?;

    let (tx, mut rx) = mpsc::channel::<Vec<DiscoveredAsset>>(32);

    let progress = Arc::new(ScanProgressState {
        files_found: AtomicUsize::new(0),
        files_inserted: AtomicUsize::new(0),
        zips_scanned: AtomicUsize::new(0),
        discovery_complete: AtomicBool::new(false),
    });

    // Spawn discovery on a blocking thread so it doesn't stall the async runtime
    let discover_app = app.clone();
    let discover_progress = progress.clone();
    let discovery_handle = tokio::task::spawn_blocking(move || {
        discover_files_streaming(&discover_app, &root_path_buf, tx, &discover_progress)
    });

    // Receive chunks and insert them as they arrive
    let pool = state.pool.clone();
    let insert_progress = progress.clone();
    let insert_app = app.clone();
    let mut last_emit = Instant::now();

    while let Some(chunk) = rx.recv().await {
        let chunk_len = chunk.len();
        insert_asset_chunk(&pool, &chunk).await?;
        let total_inserted =
            insert_progress
                .files_inserted
                .fetch_add(chunk_len, Ordering::Relaxed)
                + chunk_len;

        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let found = insert_progress.files_found.load(Ordering::Relaxed);
            let zips = insert_progress.zips_scanned.load(Ordering::Relaxed);
            let done = insert_progress.discovery_complete.load(Ordering::Relaxed);
            let _ = insert_app.emit(
                "scan-progress",
                ScanProgress {
                    phase: "scanning".to_string(),
                    files_found: found,
                    files_inserted: total_inserted,
                    files_total: if done { found } else { 0 },
                    zips_scanned: zips,
                    current_path: None,
                },
            );
            last_emit = Instant::now();
        }
    }

    // Discovery finished (tx dropped) — check for errors
    discovery_handle
        .await
        .map_err(|e| format!("Discovery task panicked: {}", e))?
        .map_err(|e| e)?;

    // Emit completion
    let total_found = progress.files_found.load(Ordering::Relaxed);
    let total_inserted = progress.files_inserted.load(Ordering::Relaxed);
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            phase: "complete".to_string(),
            files_found: total_found,
            files_inserted: total_inserted,
            files_total: total_found,
            zips_scanned: progress.zips_scanned.load(Ordering::Relaxed),
            current_path: None,
        },
    );

    update_session_total(&state, session_id, total_found).await?;
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
        "INSERT INTO scan_sessions (root_path, started_at) VALUES (?1, ?2)",
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

/// Stream-discover all supported asset files, sending chunks through the channel.
/// Runs on a blocking thread via spawn_blocking.
fn discover_files_streaming(
    app: &AppHandle,
    root_path: &Path,
    tx: mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
) -> Result<(), String> {
    let mut chunk = Vec::with_capacity(CHUNK_SIZE);
    let mut last_emit = Instant::now();

    // Initial progress event
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            phase: "scanning".to_string(),
            files_found: 0,
            files_inserted: 0,
            files_total: 0,
            zips_scanned: 0,
            current_path: Some(root_path.to_string_lossy().to_string()),
        },
    );

    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

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

            if let Some(asset_type) = detect_asset_type(path) {
                let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;

                chunk.push(DiscoveredAsset {
                    filename: filename.to_string(),
                    path: path.to_path_buf(),
                    zip_entry: None,
                    format: ext_str.to_string(),
                    asset_type,
                    file_size: metadata.len() as i64,
                });
                progress.files_found.fetch_add(1, Ordering::Relaxed);

                if chunk.len() >= CHUNK_SIZE {
                    let batch = std::mem::replace(&mut chunk, Vec::with_capacity(CHUNK_SIZE));
                    tx.blocking_send(batch)
                        .map_err(|_| "Insert task stopped".to_string())?;
                }
            } else if ext_str == "zip" {
                match discover_zip_streaming(path, &tx, progress, &mut chunk) {
                    Ok(()) => {
                        progress.zips_scanned.fetch_add(1, Ordering::Relaxed);
                        // Flush chunk after each zip so inserts aren't delayed
                        if !chunk.is_empty() {
                            let batch =
                                std::mem::replace(&mut chunk, Vec::with_capacity(CHUNK_SIZE));
                            tx.blocking_send(batch)
                                .map_err(|_| "Insert task stopped".to_string())?;
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to process zip file {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        // Emit progress periodically from discovery side
        if last_emit.elapsed().as_millis() >= EMIT_INTERVAL_MS {
            let found = progress.files_found.load(Ordering::Relaxed);
            let inserted = progress.files_inserted.load(Ordering::Relaxed);
            let zips = progress.zips_scanned.load(Ordering::Relaxed);
            let _ = app.emit(
                "scan-progress",
                ScanProgress {
                    phase: "scanning".to_string(),
                    files_found: found,
                    files_inserted: inserted,
                    files_total: 0,
                    zips_scanned: zips,
                    current_path: Some(path.to_string_lossy().to_string()),
                },
            );
            last_emit = Instant::now();
        }
    }

    // Send any remaining assets
    if !chunk.is_empty() {
        tx.blocking_send(chunk)
            .map_err(|_| "Insert task stopped".to_string())?;
    }

    progress.discovery_complete.store(true, Ordering::Release);
    Ok(())
}

/// Discover assets inside a zip file, streaming chunks through the channel
fn discover_zip_streaming(
    zip_path: &Path,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
    chunk: &mut Vec<DiscoveredAsset>,
) -> Result<(), String> {
    let file =
        File::open(zip_path).map_err(|e| format!("Failed to open zip: {}", e))?;

    let archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {}", e))?;

    discover_zip_recursive_streaming(archive, zip_path, "", tx, progress, chunk)
}

/// Recursively discover assets in a zip archive, streaming results
fn discover_zip_recursive_streaming<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    zip_path: &Path,
    prefix: &str,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
    chunk: &mut Vec<DiscoveredAsset>,
) -> Result<(), String> {
    // First pass: collect entry info (indices needed to avoid borrow conflicts)
    let mut entries_info: Vec<(usize, String, u64)> = Vec::new();

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        if !entry.is_dir() {
            entries_info.push((i, entry.name().to_string(), entry.size()));
        }
    }

    // Second pass: process entries
    for (idx, entry_name, entry_size) in entries_info {
        let entry_path_buf = PathBuf::from(&entry_name);

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

            if ext_str == "zip" {
                // Read nested zip into memory (unavoidable for zip format)
                let mut entry = archive
                    .by_index(idx)
                    .map_err(|e| format!("Failed to read nested zip entry: {}", e))?;

                let mut buffer = Vec::new();
                entry
                    .read_to_end(&mut buffer)
                    .map_err(|e| format!("Failed to read nested zip content: {}", e))?;

                let cursor = Cursor::new(buffer);
                match ZipArchive::new(cursor) {
                    Ok(nested_archive) => {
                        let nested_prefix = format!("{}{}/", prefix, entry_name);

                        if let Err(e) = discover_zip_recursive_streaming(
                            nested_archive,
                            zip_path,
                            &nested_prefix,
                            tx,
                            progress,
                            chunk,
                        ) {
                            eprintln!(
                                "Warning: Failed to process nested zip {}{}: {}",
                                prefix, entry_name, e
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to open nested zip {}{}: {}",
                            prefix, entry_name, e
                        );
                    }
                }
            } else if let Some(asset_type) = detect_asset_type_from_ext(&ext_str) {
                let full_entry_path = format!("{}{}", prefix, entry_name);

                chunk.push(DiscoveredAsset {
                    filename: filename.to_string(),
                    path: zip_path.to_path_buf(),
                    zip_entry: Some(full_entry_path),
                    format: ext_str.to_string(),
                    asset_type,
                    file_size: entry_size as i64,
                });
                progress.files_found.fetch_add(1, Ordering::Relaxed);

                if chunk.len() >= CHUNK_SIZE {
                    let batch = std::mem::replace(chunk, Vec::with_capacity(CHUNK_SIZE));
                    tx.blocking_send(batch)
                        .map_err(|_| "Insert task stopped".to_string())?;
                }
            }
        }
    }

    Ok(())
}

/// Insert a chunk of assets in a single transaction
async fn insert_asset_chunk(
    pool: &sqlx::SqlitePool,
    assets: &[DiscoveredAsset],
) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    for asset in assets {
        let path_str = asset.path.to_string_lossy().to_string();

        sqlx::query(
            "INSERT OR IGNORE INTO assets (
                filename, path, zip_entry, asset_type, format, file_size,
                created_at, modified_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
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
