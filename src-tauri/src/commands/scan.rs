use crate::{database, models::*, AppState};
use rusqlite;
use serde::Serialize;
use serde_json;
use sqlx;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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
    pub warnings: Vec<String>,
    /// Root folder path — used to distinguish concurrent scans on the frontend
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_path: Option<String>,
}

/// Supported image extensions
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Supported audio extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac"];

/// Number of assets per chunk sent through the channel
const CHUNK_SIZE: usize = 1000;

/// Minimum interval between progress event emissions
const EMIT_INTERVAL_MS: u128 = 100;

/// Number of assets per FTS indexing batch (for progress reporting)
const FTS_BATCH_SIZE: i64 = 50_000;

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
    pub warnings: std::sync::Mutex<Vec<String>>,
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

    sqlx::query("INSERT OR IGNORE INTO source_folders (path, label, added_at) VALUES (?1, ?2, ?3)")
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

    // Snapshot max asset id before insertion — used to scope FTS population
    let max_id_before: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM assets")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let (tx, mut rx) = mpsc::channel::<Vec<DiscoveredAsset>>(32);

    let progress = Arc::new(ScanProgressState {
        files_found: AtomicUsize::new(0),
        files_inserted: AtomicUsize::new(0),
        zips_scanned: AtomicUsize::new(0),
        discovery_complete: AtomicBool::new(false),
        warnings: std::sync::Mutex::new(Vec::new()),
    });

    // Spawn discovery on a blocking thread so it doesn't stall the async runtime
    let discover_app = app.clone();
    let discover_progress = progress.clone();
    let discover_folder_path = normalized_path.clone();
    let discovery_handle = tokio::task::spawn_blocking(move || {
        discover_files_streaming(
            &discover_app,
            &root_path_buf,
            folder_id,
            tx,
            &discover_progress,
            "scan-progress",
            &search_excludes,
            Some(&discover_folder_path),
        )
    });

    // Synchronous bulk insertion via rusqlite on a blocking thread.
    // Bypasses sqlx async overhead — prepare once, rebind+step+reset in a tight loop.
    let pool = state.pool.clone();
    let insert_progress = progress.clone();
    let db_path = state.db_path.clone();
    let insertion_handle = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=30000; PRAGMA wal_autocheckpoint=0;",
        )
        .map_err(|e| e.to_string())?;
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as i64;

        let mut stmt = conn
            .prepare(
                "INSERT OR IGNORE INTO assets (
                filename, folder_id, rel_path, zip_file, zip_entry, zip_compression,
                searchable_path, asset_type, format, file_size, fs_modified_at,
                created_at, modified_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )
            .map_err(|e| e.to_string())?;

        while let Some(chunk) = rx.blocking_recv() {
            let chunk_len = chunk.len();
            for asset in &chunk {
                stmt.execute(rusqlite::params![
                    asset.filename,
                    asset.folder_id,
                    asset.rel_path,
                    asset.zip_file,
                    asset.zip_entry,
                    asset.zip_compression,
                    asset.searchable_path,
                    asset.asset_type.as_str(),
                    asset.format,
                    asset.file_size,
                    asset.fs_modified_at,
                    now,
                    now,
                ])
                .map_err(|e| e.to_string())?;
            }
            insert_progress
                .files_inserted
                .fetch_add(chunk_len, Ordering::Relaxed);
        }

        drop(stmt);
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(())
    });

    // Emit progress events while discovery + insertion run concurrently
    let emit_progress = progress.clone();
    let emit_app = app.clone();
    let emit_folder_path = normalized_path.clone();
    let progress_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(EMIT_INTERVAL_MS as u64)).await;
            let found = emit_progress.files_found.load(Ordering::Relaxed);
            let inserted = emit_progress.files_inserted.load(Ordering::Relaxed);
            let done = emit_progress.discovery_complete.load(Ordering::Relaxed);
            let zips = emit_progress.zips_scanned.load(Ordering::Relaxed);
            let _ = emit_app.emit(
                "scan-progress",
                ScanProgress {
                    phase: "scanning".to_string(),
                    files_found: found,
                    files_inserted: inserted,
                    files_total: if done { found } else { 0 },
                    zips_scanned: zips,
                    current_path: None,
                    warnings: vec![],
                    folder_path: Some(emit_folder_path.clone()),
                },
            );
        }
    });

    // Insertion finishes when discovery drops tx (channel closes)
    insertion_handle
        .await
        .map_err(|e| format!("Insertion task panicked: {}", e))?
        .map_err(|e| e)?;
    progress_handle.abort();

    // Discovery should already be done — check for errors
    discovery_handle
        .await
        .map_err(|e| format!("Discovery task panicked: {}", e))?
        .map_err(|e| e)?;

    let warnings: Vec<String> = progress
        .warnings
        .lock()
        .map(|mut w| w.drain(..).collect())
        .unwrap_or_default();

    // Bulk-populate FTS indexes for newly inserted assets (INSERT trigger removed
    // for performance — trigram+unicode61 per-row indexing was the biggest write
    // amplifier). Scoped by folder_id + max_id to be safe with concurrent scans.
    // Batched with progress events so the UI doesn't appear stuck.
    populate_fts_batched(
        &app,
        &pool,
        folder_id,
        max_id_before,
        Some(&normalized_path),
    )
    .await?;

    // Populate precomputed directory tree for instant folder browsing (non-fatal)
    if let Err(e) = populate_directories(&pool, &state.db_path, folder_id).await {
        eprintln!(
            "[Scan] Failed to populate directories for folder {}: {}",
            folder_id, e
        );
    }

    // WAL auto-checkpoint is disabled globally (via after_connect); run a
    // TRUNCATE checkpoint to flush WAL → .db and free disk space.
    // TRUNCATE waits for readers to finish (frontend reads are short), so
    // a single call is sufficient — no delayed retry needed.
    database::checkpoint_truncate(&pool)
        .await
        .map_err(|e| e.to_string())?;

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
            warnings: warnings.clone(),
            folder_path: Some(normalized_path.clone()),
        },
    );

    update_session_total(&state, session_id, total_found).await?;
    update_session_status(&state, session_id, "complete").await?;

    // Encode warnings as JSON (NULL if empty)
    let warnings_json: Option<String> = if warnings.is_empty() {
        None
    } else {
        serde_json::to_string(&warnings).ok()
    };

    // Update source folder stats and persist warnings
    sqlx::query(
        "UPDATE source_folders SET last_scanned_at = ?1, asset_count = (SELECT COUNT(*) FROM assets WHERE folder_id = ?2), scan_warnings = ?3 WHERE id = ?2",
    )
    .bind(now)
    .bind(folder_id)
    .bind(warnings_json)
    .execute(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(session_id)
}

/// Create a new scan session
async fn create_scan_session(
    state: &State<'_, AppState>,
    source_folder_id: i64,
) -> Result<i64, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let result =
        sqlx::query("INSERT INTO scan_sessions (source_folder_id, started_at) VALUES (?1, ?2)")
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
///
/// Uses rayon::scope to parallelize ZIP decompression: the filesystem walk runs on
/// the calling thread while each outer ZIP file is processed in a rayon task. Nested
/// ZIPs within each outer ZIP are also spawned as separate rayon tasks, each opening
/// its own file handle. Memory is bounded via ZipCache's LRU eviction budget.
pub(crate) fn discover_files_streaming(
    app: &AppHandle,
    root_path: &Path,
    folder_id: i64,
    tx: mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &ScanProgressState,
    event_name: &str,
    search_excludes: &std::collections::HashSet<String>,
    folder_path: Option<&str>,
) -> Result<(), String> {
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
            warnings: vec![],
            folder_path: folder_path.map(|s| s.to_string()),
        },
    );

    // Collect the first fatal error from any rayon task (e.g. channel closed)
    let fatal_error: Mutex<Option<String>> = Mutex::new(None);

    rayon::scope(|s| {
        // Rebind as a reference so move closures capture &Mutex (Copy) not Mutex
        let fatal_error = &fatal_error;
        let mut chunk = Vec::with_capacity(CHUNK_SIZE);
        let mut last_emit = Instant::now();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().unwrap_or_default().to_string_lossy();

            // Skip macOS metadata files (._filename) and hidden files
            if filename.starts_with("._") || filename.starts_with('.') {
                continue;
            }

            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();

                if let Some(asset_type) = detect_asset_type(path) {
                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to read metadata for {}: {}",
                                path.display(),
                                e
                            );
                            let rel = path
                                .strip_prefix(root_path)
                                .map(|p| p.to_string_lossy().replace('\\', "/"))
                                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
                            if let Ok(mut w) = progress.warnings.lock() {
                                w.push(format!("{}: {}", rel, e));
                            }
                            continue;
                        }
                    };

                    let rel_path = compute_rel_path(root_path, path);
                    let fs_modified_at = metadata
                        .modified()
                        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
                        .unwrap_or(0);

                    let searchable_path =
                        compute_searchable_path(&rel_path, None, None, search_excludes);
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
                        if tx.blocking_send(batch).is_err() {
                            set_fatal_error(&fatal_error, "Insert task stopped".to_string());
                            return;
                        }
                    }
                } else if ext_str == "zip" {
                    let zip_path = path.to_path_buf();
                    let zip_rel_path = compute_rel_path(root_path, path);
                    let zip_filename = filename.to_string();
                    let zip_mtime = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
                        .unwrap_or(0);
                    let tx_zip = tx.clone();

                    s.spawn(move |s| {
                        discover_zip_parallel(
                            s,
                            &zip_path,
                            folder_id,
                            &zip_rel_path,
                            &zip_filename,
                            zip_mtime,
                            &tx_zip,
                            progress,
                            search_excludes,
                            &fatal_error,
                        );
                    });
                }
            }

            // Emit progress periodically from walk
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
                        warnings: vec![],
                        folder_path: folder_path.map(|s| s.to_string()),
                    },
                );
                last_emit = Instant::now();
            }
        }

        // Send remaining regular file assets
        if !chunk.is_empty() {
            if tx.blocking_send(chunk).is_err() {
                set_fatal_error(&fatal_error, "Insert task stopped".to_string());
            }
        }
    });
    // rayon::scope blocks until all tasks (walk + all ZIP tasks) complete

    // Check for fatal errors from any task
    let error = fatal_error
        .into_inner()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    if let Some(err) = error {
        return Err(err);
    }

    progress.discovery_complete.store(true, Ordering::Release);
    Ok(())
}

/// Set a fatal error, keeping only the first one.
fn set_fatal_error(fatal_error: &Mutex<Option<String>>, msg: String) {
    if let Ok(mut e) = fatal_error.lock() {
        if e.is_none() {
            *e = Some(msg);
        }
    }
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

/// Process an outer ZIP file in a rayon task: enumerate entries, spawn nested ZIP tasks.
fn discover_zip_parallel<'scope>(
    scope: &rayon::Scope<'scope>,
    zip_path: &Path,
    folder_id: i64,
    rel_path: &str,
    zip_filename: &str,
    zip_mtime: i64,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &'scope ScanProgressState,
    search_excludes: &'scope std::collections::HashSet<String>,
    fatal_error: &'scope Mutex<Option<String>>,
) {
    let file = match File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Failed to open zip {}: {}", zip_path.display(), e);
            let rel = if rel_path.is_empty() {
                zip_filename.to_string()
            } else {
                format!("{}/{}", rel_path, zip_filename)
            };
            if let Ok(mut w) = progress.warnings.lock() {
                w.push(format!("{}: {}", rel, e));
            }
            return;
        }
    };
    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "Warning: Failed to read zip archive {}: {}",
                zip_path.display(),
                e
            );
            let rel = if rel_path.is_empty() {
                zip_filename.to_string()
            } else {
                format!("{}/{}", rel_path, zip_filename)
            };
            if let Ok(mut w) = progress.warnings.lock() {
                w.push(format!("{}: {}", rel, e));
            }
            return;
        }
    };

    let outer_zip_path_str = zip_path.to_string_lossy().replace('\\', "/");

    enumerate_zip_entries_parallel(
        scope,
        &mut archive,
        &outer_zip_path_str,
        "",
        folder_id,
        rel_path,
        zip_filename,
        zip_mtime,
        "",
        tx,
        progress,
        search_excludes,
        fatal_error,
    );

    progress.zips_scanned.fetch_add(1, Ordering::Relaxed);
}

/// Process a nested ZIP via ZipCache (memory-bounded), spawning tasks for deeper nesting.
fn discover_nested_zip_parallel<'scope>(
    scope: &rayon::Scope<'scope>,
    outer_zip_path: &str,
    cache_path: &str,
    size_hint: u64,
    folder_id: i64,
    rel_path: &str,
    zip_filename: &str,
    zip_mtime: i64,
    prefix: &str,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &'scope ScanProgressState,
    search_excludes: &'scope std::collections::HashSet<String>,
    fatal_error: &'scope Mutex<Option<String>>,
) {
    // Load through ZipCache for memory bounding and cache warming
    let (bytes, _guard) =
        match crate::zip_cache::load_for_scan(outer_zip_path, cache_path, size_hint) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to load nested zip {}/{}: {}",
                    outer_zip_path, cache_path, e
                );
                let rel = if rel_path.is_empty() {
                    format!("{}/{}", zip_filename, cache_path)
                } else {
                    format!("{}/{}/{}", rel_path, zip_filename, cache_path)
                };
                if let Ok(mut w) = progress.warnings.lock() {
                    w.push(format!("{}: {}", rel, e));
                }
                return;
            }
        };

    let mut archive = match ZipArchive::new(Cursor::new(bytes.as_slice())) {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "Warning: Failed to open nested zip {}/{}: {}",
                outer_zip_path, cache_path, e
            );
            let rel = if rel_path.is_empty() {
                format!("{}/{}", zip_filename, cache_path)
            } else {
                format!("{}/{}/{}", rel_path, zip_filename, cache_path)
            };
            if let Ok(mut w) = progress.warnings.lock() {
                w.push(format!("{}: {}", rel, e));
            }
            return;
        }
    };

    enumerate_zip_entries_parallel(
        scope,
        &mut archive,
        outer_zip_path,
        cache_path,
        folder_id,
        rel_path,
        zip_filename,
        zip_mtime,
        prefix,
        tx,
        progress,
        search_excludes,
        fatal_error,
    );
}

/// Shared enumeration logic for both outer and nested ZIPs.
/// Enumerates entries, sends media assets through the channel, and spawns rayon
/// tasks for any nested ZIP entries found.
fn enumerate_zip_entries_parallel<'scope, R: Read + Seek>(
    scope: &rayon::Scope<'scope>,
    archive: &mut ZipArchive<R>,
    outer_zip_path: &str,
    cache_path: &str,
    folder_id: i64,
    rel_path: &str,
    zip_filename: &str,
    zip_mtime: i64,
    prefix: &str,
    tx: &mpsc::Sender<Vec<DiscoveredAsset>>,
    progress: &'scope ScanProgressState,
    search_excludes: &'scope std::collections::HashSet<String>,
    fatal_error: &'scope Mutex<Option<String>>,
) {
    // First pass: collect entry metadata to avoid borrow conflicts with archive
    let mut entries: Vec<(String, u64, &'static str)> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            if !entry.is_dir() {
                entries.push((
                    entry.name().to_string(),
                    entry.size(),
                    compression_method_str(entry.compression()),
                ));
            }
        }
    }

    // Second pass: process entries (archive no longer borrowed)
    let mut chunk = Vec::with_capacity(CHUNK_SIZE);

    for (entry_name, entry_size, entry_compression) in entries {
        if fatal_error.lock().map_or(false, |e| e.is_some()) {
            return;
        }

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
                // Compute paths for the nested ZIP task
                let nested_cache_path = if cache_path.is_empty() {
                    entry_name.clone()
                } else {
                    format!("{}/{}", cache_path, entry_name)
                };
                let nested_prefix = format!("{}{}/", prefix, entry_name);
                let outer = outer_zip_path.to_string();
                let tx_nested = tx.clone();
                let rel = rel_path.to_string();
                let zfn = zip_filename.to_string();

                scope.spawn(move |scope| {
                    discover_nested_zip_parallel(
                        scope,
                        &outer,
                        &nested_cache_path,
                        entry_size,
                        folder_id,
                        &rel,
                        &zfn,
                        zip_mtime,
                        &nested_prefix,
                        &tx_nested,
                        progress,
                        search_excludes,
                        fatal_error,
                    );
                });
            } else if let Some(asset_type) = detect_asset_type_from_ext(&ext_str) {
                let full_entry_path = format!("{}{}", prefix, entry_name);
                let searchable_path = compute_searchable_path(
                    rel_path,
                    Some(zip_filename),
                    Some(&full_entry_path),
                    search_excludes,
                );

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
                    let batch = std::mem::replace(&mut chunk, Vec::with_capacity(CHUNK_SIZE));
                    if tx.blocking_send(batch).is_err() {
                        set_fatal_error(fatal_error, "Insert task stopped".to_string());
                        return;
                    }
                }
            }
        }
    }

    // Flush remaining assets in this task's chunk
    if !chunk.is_empty() {
        if tx.blocking_send(chunk).is_err() {
            set_fatal_error(fatal_error, "Insert task stopped".to_string());
        }
    }
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

/// Bulk-populate both FTS indexes for newly inserted assets in a folder,
/// in batches of FTS_BATCH_SIZE with progress events.
/// Uses folder_id + min_id_exclusive to scope to only this scan's new assets,
/// which is safe when multiple scans for different folders run concurrently.
async fn populate_fts_batched(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    folder_id: i64,
    min_id_exclusive: i64,
    folder_path: Option<&str>,
) -> Result<(), String> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE folder_id = ?1 AND id > ?2")
            .bind(folder_id)
            .bind(min_id_exclusive)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    if total == 0 {
        return Ok(());
    }

    let max_id: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(id), 0) FROM assets WHERE folder_id = ?1 AND id > ?2",
    )
    .bind(folder_id)
    .bind(min_id_exclusive)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut current_min = min_id_exclusive;
    let mut indexed: usize = 0;

    while current_min < max_id {
        let batch_max = (current_min + FTS_BATCH_SIZE).min(max_id);

        let result = sqlx::query(
            "INSERT INTO assets_fts_sub(rowid, filename, searchable_path)
             SELECT id, filename, searchable_path FROM assets
             WHERE folder_id = ?1 AND id > ?2 AND id <= ?3",
        )
        .bind(folder_id)
        .bind(current_min)
        .bind(batch_max)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        let batch_count = result.rows_affected() as usize;

        sqlx::query(
            "INSERT INTO assets_fts_word(rowid, filename, searchable_path)
             SELECT id, filename, searchable_path FROM assets
             WHERE folder_id = ?1 AND id > ?2 AND id <= ?3",
        )
        .bind(folder_id)
        .bind(current_min)
        .bind(batch_max)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        indexed += batch_count;

        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                phase: "indexing".to_string(),
                files_found: total as usize,
                files_inserted: indexed,
                files_total: total as usize,
                zips_scanned: 0,
                current_path: None,
                warnings: vec![],
                folder_path: folder_path.map(|s| s.to_string()),
            },
        );

        current_min = batch_max;
    }

    Ok(())
}

/// Populate the `directories` table for a folder by querying the assets table.
/// Runs after asset insertion (scan or rescan). Replaces all directory rows for
/// the folder — safe because the directory set is small (hundreds to low thousands).
pub(crate) async fn populate_directories(
    pool: &sqlx::SqlitePool,
    db_path: &str,
    folder_id: i64,
) -> Result<(), String> {
    // Collect directory data from assets using sqlx (async-friendly)
    // 1. Filesystem directories: distinct rel_path values (non-zip assets)
    let fs_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT rel_path, COUNT(*) as cnt FROM assets
         WHERE folder_id = ?1 AND zip_file IS NULL
         GROUP BY rel_path",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("populate_directories: fs query failed: {}", e))?;

    // 2. Zip archives: distinct (rel_path, zip_file) with asset counts
    let zip_rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT rel_path, zip_file, COUNT(*) as cnt FROM assets
         WHERE folder_id = ?1 AND zip_file IS NOT NULL
         GROUP BY rel_path, zip_file",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("populate_directories: zip query failed: {}", e))?;

    // 3. Zip-internal directories: distinct (rel_path, zip_file, zip_entry) for dir extraction
    let zip_entry_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT rel_path, zip_file, zip_entry FROM assets
         WHERE folder_id = ?1 AND zip_file IS NOT NULL AND zip_entry IS NOT NULL",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("populate_directories: zip entry query failed: {}", e))?;

    // Do the heavy lifting on a blocking thread with rusqlite for fast bulk insert
    let db_path = db_path.to_string();
    tokio::task::spawn_blocking(move || {
        populate_directories_blocking(&db_path, folder_id, &fs_rows, &zip_rows, &zip_entry_rows)
    })
    .await
    .map_err(|e| format!("populate_directories: task panicked: {}", e))?
}

/// Key for looking up a directory node's inserted row ID
#[derive(Hash, Eq, PartialEq, Clone)]
enum DirKey {
    /// Filesystem directory: (rel_path,)
    Fs(String),
    /// ZIP archive node: (rel_path, zip_file)
    Zip(String, String),
    /// Directory inside a ZIP: (rel_path, zip_file, zip_prefix)
    ZipDir(String, String, String),
}

fn populate_directories_blocking(
    db_path: &str,
    folder_id: i64,
    fs_rows: &[(String, i64)],
    zip_rows: &[(String, String, i64)],
    zip_entry_rows: &[(String, String, String)],
) -> Result<(), String> {
    use std::collections::HashMap;

    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=30000; PRAGMA wal_autocheckpoint=0;",
    )
    .map_err(|e| e.to_string())?;
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

    // Clear existing directories for this folder
    conn.execute("DELETE FROM directories WHERE folder_id = ?1", [folder_id])
        .map_err(|e| e.to_string())?;

    let mut id_map: HashMap<DirKey, i64> = HashMap::new();
    // Track asset counts per directory: (dir_key -> count of direct assets)
    let mut asset_counts: HashMap<DirKey, i64> = HashMap::new();
    // Track child counts: (dir_key -> count of direct children)
    let mut child_counts: HashMap<DirKey, i64> = HashMap::new();

    // --- Build filesystem directory nodes ---
    // From fs_rows, extract all directory segments and count assets
    for (rel_path, count) in fs_rows {
        if rel_path.is_empty() {
            // Assets at folder root — no directory node needed (root is the source folder)
            continue;
        }
        let key = DirKey::Fs(rel_path.clone());
        *asset_counts.entry(key).or_insert(0) += count;

        // Ensure all ancestor directories exist
        let segments: Vec<&str> = rel_path.split('/').collect();
        for depth in 0..segments.len() {
            let ancestor_path = segments[..=depth].join("/");
            let ancestor_key = DirKey::Fs(ancestor_path);
            // Just ensure it exists in asset_counts (will get 0 if no direct assets)
            asset_counts.entry(ancestor_key).or_insert(0);
        }
    }

    // Also ensure filesystem directories exist for any rel_path that has zip files
    for (rel_path, _, _) in zip_rows {
        if !rel_path.is_empty() {
            let segments: Vec<&str> = rel_path.split('/').collect();
            for depth in 0..segments.len() {
                let ancestor_path = segments[..=depth].join("/");
                asset_counts.entry(DirKey::Fs(ancestor_path)).or_insert(0);
            }
        }
    }

    // --- Build zip archive nodes ---
    for (rel_path, zip_file, count) in zip_rows {
        let key = DirKey::Zip(rel_path.clone(), zip_file.clone());
        *asset_counts.entry(key).or_insert(0) += count;
    }

    // --- Build zip-internal directory nodes ---
    // Extract leaf directory from each zip_entry, then ensure all ancestor zip dirs exist
    {
        // Collect unique (rel_path, zip_file, leaf_dir) tuples
        let mut zip_dirs: HashMap<(String, String), std::collections::HashSet<String>> =
            HashMap::new();
        for (rel_path, zip_file, zip_entry) in zip_entry_rows {
            if let Some(last_slash) = zip_entry.rfind('/') {
                let leaf_dir = &zip_entry[..last_slash];
                zip_dirs
                    .entry((rel_path.clone(), zip_file.clone()))
                    .or_default()
                    .insert(leaf_dir.to_string());
            }
        }

        for ((rel_path, zip_file), leaf_dirs) in &zip_dirs {
            for leaf_dir in leaf_dirs {
                // Create all ancestor zip-internal directories
                let segments: Vec<&str> = leaf_dir.split('/').collect();
                for depth in 0..segments.len() {
                    let prefix = segments[..=depth].join("/") + "/";
                    let key = DirKey::ZipDir(rel_path.clone(), zip_file.clone(), prefix);
                    asset_counts.entry(key).or_insert(0);
                }
            }
        }

        // Count assets per zip-internal directory
        for (rel_path, zip_file, zip_entry) in zip_entry_rows {
            // The directory containing this entry
            if let Some(last_slash) = zip_entry.rfind('/') {
                let dir_prefix = zip_entry[..last_slash].to_string() + "/";
                let key = DirKey::ZipDir(rel_path.clone(), zip_file.clone(), dir_prefix);
                *asset_counts.entry(key).or_insert(0) += 1;
            } else {
                // File at zip root — count toward the zip archive node itself
                // (already counted in zip_rows)
            }
        }
    }

    // --- Compute child counts ---
    // A child of DirKey::Fs("a/b") is DirKey::Fs("a/b/x") where x has no more slashes
    // A child of DirKey::Zip(rp, zf) is DirKey::ZipDir(rp, zf, "x/") where x has no slashes
    // A child of DirKey::ZipDir(rp, zf, "a/b/") is DirKey::ZipDir(rp, zf, "a/b/x/")
    // A child of DirKey::Fs("") (root) is DirKey::Fs("x") and DirKey::Zip("", zf)
    let all_keys: Vec<DirKey> = asset_counts.keys().cloned().collect();
    for key in &all_keys {
        let parent_key = match key {
            DirKey::Fs(rel_path) => {
                if let Some(last_slash) = rel_path.rfind('/') {
                    Some(DirKey::Fs(rel_path[..last_slash].to_string()))
                } else {
                    None // direct child of folder root — counted on source_folders
                }
            }
            DirKey::Zip(rel_path, _zip_file) => {
                if rel_path.is_empty() {
                    None // zip at folder root
                } else {
                    Some(DirKey::Fs(rel_path.clone()))
                }
            }
            DirKey::ZipDir(rel_path, zip_file, zip_prefix) => {
                // Parent is the zip prefix with one fewer segment
                let trimmed = zip_prefix.trim_end_matches('/');
                if let Some(last_slash) = trimmed.rfind('/') {
                    Some(DirKey::ZipDir(
                        rel_path.clone(),
                        zip_file.clone(),
                        trimmed[..last_slash].to_string() + "/",
                    ))
                } else {
                    // Direct child of zip root
                    Some(DirKey::Zip(rel_path.clone(), zip_file.clone()))
                }
            }
        };
        if let Some(pk) = parent_key {
            *child_counts.entry(pk).or_insert(0) += 1;
        }
    }

    // --- Insert all nodes, parents first ---
    // Sort by depth (number of path segments) to ensure parents are inserted first
    let mut sorted_keys: Vec<DirKey> = asset_counts.keys().cloned().collect();
    sorted_keys.sort_by_key(|k| match k {
        DirKey::Fs(rp) => (0, rp.matches('/').count(), rp.clone()),
        DirKey::Zip(rp, zf) => (1, rp.matches('/').count(), format!("{}\0{}", rp, zf)),
        DirKey::ZipDir(rp, zf, zp) => (
            2,
            rp.matches('/').count() + zp.matches('/').count(),
            format!("{}\0{}\0{}", rp, zf, zp),
        ),
    });

    // --- Compute cumulative asset counts ---
    // Iterate deepest-first, bubbling each node's count up to its parent.
    let mut cumulative_counts: HashMap<DirKey, i64> = asset_counts.clone();
    for key in sorted_keys.iter().rev() {
        let cum = *cumulative_counts.get(key).unwrap_or(&0);
        let parent_key = match key {
            DirKey::Fs(rel_path) => {
                if let Some(last_slash) = rel_path.rfind('/') {
                    Some(DirKey::Fs(rel_path[..last_slash].to_string()))
                } else {
                    None
                }
            }
            DirKey::Zip(rel_path, _zip_file) => {
                if rel_path.is_empty() {
                    None
                } else {
                    Some(DirKey::Fs(rel_path.clone()))
                }
            }
            DirKey::ZipDir(rel_path, zip_file, zip_prefix) => {
                let trimmed = zip_prefix.trim_end_matches('/');
                if let Some(last_slash) = trimmed.rfind('/') {
                    Some(DirKey::ZipDir(
                        rel_path.clone(),
                        zip_file.clone(),
                        trimmed[..last_slash].to_string() + "/",
                    ))
                } else {
                    Some(DirKey::Zip(rel_path.clone(), zip_file.clone()))
                }
            }
        };
        if let Some(pk) = parent_key {
            *cumulative_counts.entry(pk).or_insert(0) += cum;
        }
    }

    let mut stmt = conn
        .prepare(
            "INSERT INTO directories (folder_id, parent_id, name, rel_path, zip_file, zip_prefix, asset_count, cumulative_asset_count, child_count, dir_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .map_err(|e| e.to_string())?;

    for key in &sorted_keys {
        let (parent_id, name, rel_path, zip_file, zip_prefix, dir_type) = match key {
            DirKey::Fs(rp) => {
                let name = rp.rsplit('/').next().unwrap_or(rp).to_string();
                let parent_key = if let Some(last_slash) = rp.rfind('/') {
                    Some(DirKey::Fs(rp[..last_slash].to_string()))
                } else {
                    None
                };
                let parent_id = parent_key.and_then(|pk| id_map.get(&pk).copied());
                (
                    parent_id,
                    name,
                    rp.clone(),
                    None::<String>,
                    None::<String>,
                    "dir",
                )
            }
            DirKey::Zip(rp, zf) => {
                let parent_key = if rp.is_empty() {
                    None
                } else {
                    Some(DirKey::Fs(rp.clone()))
                };
                let parent_id = parent_key.and_then(|pk| id_map.get(&pk).copied());
                (
                    parent_id,
                    zf.clone(),
                    rp.clone(),
                    Some(zf.clone()),
                    None::<String>,
                    "zip",
                )
            }
            DirKey::ZipDir(rp, zf, zp) => {
                let trimmed = zp.trim_end_matches('/');
                let name = trimmed.rsplit('/').next().unwrap_or(trimmed).to_string();
                let parent_key = if let Some(last_slash) = trimmed.rfind('/') {
                    DirKey::ZipDir(
                        rp.clone(),
                        zf.clone(),
                        trimmed[..last_slash].to_string() + "/",
                    )
                } else {
                    DirKey::Zip(rp.clone(), zf.clone())
                };
                let parent_id = id_map.get(&parent_key).copied();
                (
                    parent_id,
                    name,
                    rp.clone(),
                    Some(zf.clone()),
                    Some(zp.clone()),
                    "zipdir",
                )
            }
        };

        let ac = asset_counts.get(key).copied().unwrap_or(0);
        let cac = cumulative_counts.get(key).copied().unwrap_or(0);
        let cc = child_counts.get(key).copied().unwrap_or(0);

        stmt.execute(rusqlite::params![
            folder_id, parent_id, name, rel_path, zip_file, zip_prefix, ac, cac, cc, dir_type,
        ])
        .map_err(|e| e.to_string())?;

        let row_id = conn.last_insert_rowid();
        id_map.insert(key.clone(), row_id);
    }

    drop(stmt);
    conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;

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
