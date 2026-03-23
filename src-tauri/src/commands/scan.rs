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
#[derive(Debug, Clone)]
pub(crate) struct DiscoveredAsset {
    pub filename: String,
    pub folder_id: i64,
    pub rel_path: String,
    pub zip_file: Option<String>,
    pub zip_entry: Option<String>,
    pub zip_compression: Option<String>,
    pub searchable_path: String,
    pub format: String,
    pub asset_type: AssetType,
    pub file_size: i64,
    pub fs_modified_at: i64,
}

/// Shared progress counters between discovery and insertion tasks
pub(crate) struct ScanProgressState {
    pub files_found: AtomicUsize,
    pub files_inserted: AtomicUsize,
    pub zips_scanned: AtomicUsize,
    pub discovery_complete: AtomicBool,
}

/// Add a folder as a source folder and scan it for assets.
///
/// Discovery runs on a blocking thread and streams asset chunks through a channel.
/// Insertion runs concurrently on the async runtime, inserting chunks as they arrive.
#[tauri::command]
pub async fn add_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<i64, String> {
    let root_path_buf = PathBuf::from(&path);

    if !root_path_buf.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    // Normalize path to forward slashes
    let normalized_path = path.replace('\\', "/");

    // Insert or get source folder
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let label = root_path_buf
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    sqlx::query(
        "INSERT OR IGNORE INTO source_folders (path, label, added_at) VALUES (?1, ?2, ?3)",
    )
    .bind(&normalized_path)
    .bind(&label)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let folder_id: i64 = sqlx::query_scalar("SELECT id FROM source_folders WHERE path = ?1")
        .bind(&normalized_path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let session_id = create_scan_session(&state, folder_id).await?;

    // Load folder search excludes
    let search_excludes = load_search_excludes(&state.pool, folder_id).await?;

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
        discover_files_streaming(&discover_app, &root_path_buf, folder_id, tx, &discover_progress, "scan-progress", &search_excludes)
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

    // Update source folder stats
    sqlx::query(
        "UPDATE source_folders SET last_scanned_at = ?1, asset_count = (SELECT COUNT(*) FROM assets WHERE folder_id = ?2) WHERE id = ?2",
    )
    .bind(now)
    .bind(folder_id)
    .execute(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(session_id)
}

/// Create a new scan session
async fn create_scan_session(state: &State<'_, AppState>, source_folder_id: i64) -> Result<i64, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let result = sqlx::query(
        "INSERT INTO scan_sessions (source_folder_id, started_at) VALUES (?1, ?2)",
    )
    .bind(source_folder_id)
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
pub(crate) fn discover_files_streaming(
    app: &AppHandle,
    root_path: &Path,
    folder_id: i64,
    tx: mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
    event_name: &str,
    search_excludes: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let mut chunk = Vec::with_capacity(CHUNK_SIZE);
    let mut last_emit = Instant::now();

    // Initial progress event
    let _ = app.emit(
        event_name,
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
                let metadata = entry.metadata().map_err(|e| e.to_string())?;

                // Compute rel_path: directory portion relative to root, forward slashes
                let rel_path = compute_rel_path(root_path, path);

                // Get actual filesystem mtime
                let fs_modified_at = metadata
                    .modified()
                    .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
                    .unwrap_or(0);

                let searchable_path = compute_searchable_path(&rel_path, None, None, search_excludes);
                chunk.push(DiscoveredAsset {
                    filename: filename.to_string(),
                    folder_id,
                    rel_path,
                    zip_file: None,
                    zip_entry: None,
                    zip_compression: None,
                    searchable_path,
                    format: ext_str.to_string(),
                    asset_type,
                    file_size: metadata.len() as i64,
                    fs_modified_at,
                });
                progress.files_found.fetch_add(1, Ordering::Relaxed);

                if chunk.len() >= CHUNK_SIZE {
                    let batch = std::mem::replace(&mut chunk, Vec::with_capacity(CHUNK_SIZE));
                    tx.blocking_send(batch)
                        .map_err(|_| "Insert task stopped".to_string())?;
                }
            } else if ext_str == "zip" {
                // Compute rel_path and zip_file for ZIP entries
                let zip_rel_path = compute_rel_path(root_path, path);
                let zip_filename = filename.to_string();

                // Get ZIP file mtime for all entries inside
                let zip_mtime = entry.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
                    .unwrap_or(0);

                match discover_zip_streaming(
                    path,
                    folder_id,
                    &zip_rel_path,
                    &zip_filename,
                    zip_mtime,
                    &tx,
                    progress,
                    &mut chunk,
                    search_excludes,
                ) {
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
                event_name,
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

/// Compute the relative directory path from root to the file's parent directory.
/// Returns forward-slash-separated path with no leading/trailing slashes.
/// Returns empty string if the file is directly in the root directory.
pub(crate) fn compute_rel_path(root: &Path, file_path: &Path) -> String {
    let relative = file_path
        .parent()
        .unwrap_or(file_path)
        .strip_prefix(root)
        .unwrap_or(Path::new(""));
    let s = relative.to_string_lossy().replace('\\', "/");
    // Trim trailing slash if any
    s.trim_end_matches('/').to_string()
}

/// Compute the searchable path for FTS indexing.
///
/// Takes `rel_path`, optional `zip_entry`, and a config slice of
/// `(subfolder_prefix, skip_depth)` pairs sorted by prefix length descending.
/// Finds the longest matching prefix, strips that prefix plus `skip_depth`
/// additional segments from the rel_path, appends the directory portion of
/// zip_entry (if present), and replaces path separators with spaces.
/// Compute the searchable path for FTS indexing.
///
/// `excludes` is a set of (zip_file, cumulative_path) pairs. Segments whose
/// cumulative path appears in the set are omitted from the result.
pub(crate) fn compute_searchable_path(
    rel_path: &str,
    zip_file: Option<&str>,
    zip_entry: Option<&str>,
    excludes: &std::collections::HashSet<String>,
) -> String {
    let mut result = Vec::new();

    // Reusable buffer for probing the excludes set without allocating per-lookup.
    // Format: "{zip_file_or_empty}\0{cumulative_path}"
    // Filesystem segments use prefix "\0" (empty zip_file).
    let mut probe = String::from("\0");

    // Filesystem segments: walk rel_path, skip excluded cumulative paths
    for segment in rel_path.split('/').filter(|s| !s.is_empty()) {
        if probe.len() > 1 {
            probe.push('/');
        }
        probe.push_str(segment);
        if !excludes.contains(probe.as_str()) {
            result.push(segment);
        }
    }

    // ZIP-internal directory segments (directory portion of zip_entry, before last '/')
    if let Some(entry) = zip_entry {
        if let Some(last_slash) = entry.rfind('/') {
            let dir_part = &entry[..last_slash];
            let zip_prefix = zip_file.unwrap_or("");
            probe.clear();
            probe.push_str(zip_prefix);
            probe.push('\0');
            let base_len = probe.len();
            for segment in dir_part.split('/').filter(|s| !s.is_empty()) {
                if probe.len() > base_len {
                    probe.push('/');
                }
                probe.push_str(segment);
                if !excludes.contains(probe.as_str()) {
                    result.push(segment);
                }
            }
        }
    }

    result.join(" ")
}

/// Discover assets inside a zip file, streaming chunks through the channel
fn discover_zip_streaming(
    zip_path: &Path,
    folder_id: i64,
    rel_path: &str,
    zip_filename: &str,
    zip_mtime: i64,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
    chunk: &mut Vec<DiscoveredAsset>,
    search_excludes: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let file =
        File::open(zip_path).map_err(|e| format!("Failed to open zip: {}", e))?;

    let archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {}", e))?;

    discover_zip_recursive_streaming(
        archive, folder_id, rel_path, zip_filename, zip_mtime,
        "", tx, progress, chunk, search_excludes,
    )
}

/// Recursively discover assets in a zip archive, streaming results
fn discover_zip_recursive_streaming<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    folder_id: i64,
    rel_path: &str,
    zip_filename: &str,
    zip_mtime: i64,
    prefix: &str,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
    chunk: &mut Vec<DiscoveredAsset>,
    search_excludes: &std::collections::HashSet<String>,
) -> Result<(), String> {
    // First pass: collect entry info (indices needed to avoid borrow conflicts)
    let mut entries_info: Vec<(usize, String, u64, &'static str)> = Vec::new();

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        if !entry.is_dir() {
            let compression = compression_method_str(entry.compression());
            entries_info.push((i, entry.name().to_string(), entry.size(), compression));
        }
    }

    // Second pass: process entries
    for (idx, entry_name, entry_size, entry_compression) in entries_info {
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
                            folder_id,
                            rel_path,
                            zip_filename,
                            zip_mtime,
                            &nested_prefix,
                            tx,
                            progress,
                            chunk,
                            search_excludes,
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
                let searchable_path = compute_searchable_path(rel_path, Some(zip_filename), Some(&full_entry_path), search_excludes);

                chunk.push(DiscoveredAsset {
                    filename: filename.to_string(),
                    folder_id,
                    rel_path: rel_path.to_string(),
                    zip_file: Some(zip_filename.to_string()),
                    zip_entry: Some(full_entry_path),
                    zip_compression: Some(entry_compression.to_string()),
                    searchable_path,
                    format: ext_str.to_string(),
                    asset_type,
                    file_size: entry_size as i64,
                    fs_modified_at: zip_mtime,
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
pub(crate) async fn insert_asset_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    asset: &DiscoveredAsset,
    now: i64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO assets (
            filename, folder_id, rel_path, zip_file, zip_entry, zip_compression,
            searchable_path, asset_type, format, file_size, fs_modified_at,
            created_at, modified_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )
    .bind(&asset.filename)
    .bind(asset.folder_id)
    .bind(&asset.rel_path)
    .bind(&asset.zip_file)
    .bind(&asset.zip_entry)
    .bind(&asset.zip_compression)
    .bind(&asset.searchable_path)
    .bind(asset.asset_type.as_str())
    .bind(&asset.format)
    .bind(asset.file_size)
    .bind(asset.fs_modified_at)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn insert_asset_chunk(
    pool: &sqlx::SqlitePool,
    assets: &[DiscoveredAsset],
) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    for asset in assets {
        insert_asset_row(&mut tx, asset, now).await?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Load folder search excludes as a HashSet of (zip_file, excluded_path) pairs.
/// Encode an exclude key as `"{zip_file_or_empty}\0{path}"` for O(1) borrowed lookup.
fn encode_exclude_key(zip_file: Option<&str>, path: &str) -> String {
    let prefix = zip_file.unwrap_or("");
    let mut key = String::with_capacity(prefix.len() + 1 + path.len());
    key.push_str(prefix);
    key.push('\0');
    key.push_str(path);
    key
}

pub(crate) async fn load_search_excludes(
    pool: &sqlx::SqlitePool,
    folder_id: i64,
) -> Result<std::collections::HashSet<String>, String> {
    let rows: Vec<(Option<String>, String)> = sqlx::query_as(
        "SELECT zip_file, excluded_path FROM folder_search_excludes
         WHERE source_folder_id = ?1",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|(zf, path)| encode_exclude_key(zf.as_deref(), path))
        .collect())
}

/// Detect asset type from file extension
fn detect_asset_type(path: &Path) -> Option<AssetType> {
    path.extension().and_then(|ext| {
        let ext_str = ext.to_string_lossy().to_lowercase();
        detect_asset_type_from_ext(&ext_str)
    })
}

/// Convert a ZIP compression method to a canonical lowercase string.
fn compression_method_str(method: zip::CompressionMethod) -> &'static str {
    match method {
        zip::CompressionMethod::Stored => "store",
        zip::CompressionMethod::Deflated => "deflate",
        zip::CompressionMethod::Deflate64 => "deflate64",
        zip::CompressionMethod::Bzip2 => "bzip2",
        zip::CompressionMethod::Zstd => "zstd",
        zip::CompressionMethod::Lzma => "lzma",
        _ => "unknown",
    }
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
