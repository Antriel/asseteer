//! CLAP server lifecycle management
//!
//! Starts the CLAP Python server using `uv run` (automatic Python management)
//! with fallback to a manual venv if configured.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::sync::Mutex;

use super::client::get_clap_client;
use super::logs;
use super::uv;

use once_cell::sync::Lazy;
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
pub struct ClapStartupProgress {
    pub phase: String,
    pub detail: Option<String>,
}

fn emit_startup_progress(phase: &str, detail: Option<&str>) {
    if let Some(handle) = super::get_app_handle() {
        let _ = handle.emit(
            "clap-startup-progress",
            ClapStartupProgress {
                phase: phase.to_string(),
                detail: detail.map(|s| s.to_string()),
            },
        );
    }
}

static SERVER_PROCESS: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

/// Ensures CLAP server is running, starts it if needed
pub async fn ensure_server_running() -> Result<(), String> {
    println!("[CLAP] Checking if server is running...");
    emit_startup_progress("checking", None);

    // Check if already running (quick check without lock)
    if get_clap_client().await.health_check().await.is_ok() {
        println!("[CLAP] Server already running");
        emit_startup_progress("ready", None);
        return Ok(());
    }

    println!("[CLAP] Server not running, attempting to start...");

    // Acquire lock
    let mut guard = SERVER_PROCESS.lock().await;

    // Double-check after acquiring lock
    if get_clap_client().await.health_check().await.is_ok() {
        println!("[CLAP] Server started by another task");
        emit_startup_progress("ready", None);
        return Ok(());
    }

    let clap_dir = find_clap_server_dir()?;

    let child = match start_server_process(&clap_dir).await {
        Ok(child) => child,
        Err(e) => {
            emit_startup_progress("error", Some(&e));
            return Err(e);
        }
    };

    println!("[CLAP] Process spawned with PID: {}", child.id());

    // Assign to Windows Job Object so the OS kills it if we crash
    if let Err(e) = super::job_object::assign_child_to_job(&child) {
        println!("[CLAP] Warning: could not assign to job object: {}", e);
    }

    *guard = Some(child);

    // Hold lock until server is ready — prevents concurrent callers from
    // spawning duplicate server processes during the startup window
    emit_startup_progress("waiting-for-server", Some("Starting Python server"));
    if let Err(e) = wait_for_server_ready().await {
        emit_startup_progress("error", Some(&e));
        return Err(e);
    }

    drop(guard);

    // Trigger model preload so first inference is fast
    emit_startup_progress("loading-model", Some("Loading AI model"));
    call_preload().await;

    emit_startup_progress("ready", None);

    Ok(())
}

/// Locate the clap-server directory relative to cwd.
fn find_clap_server_dir() -> Result<std::path::PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("Failed to get current dir: {}", e))?;

    // Try cwd first, then parent (Tauri runs from src-tauri/)
    if cwd.join("clap-server").exists() {
        Ok(cwd.join("clap-server"))
    } else if cwd
        .parent()
        .map(|p| p.join("clap-server").exists())
        .unwrap_or(false)
    {
        Ok(cwd.parent().unwrap().join("clap-server"))
    } else {
        Err(format!(
            "Could not find clap-server directory (cwd: {:?})",
            cwd
        ))
    }
}

/// Start the CLAP server process.
///
/// Tries `uv run` first (automatic Python management). If uv download fails
/// and a manual venv exists, falls back to using the venv directly.
/// Server output is captured to a log file in the app data directory.
async fn start_server_process(clap_dir: &std::path::Path) -> Result<Child, String> {
    let (stdout, stderr, log_path) = logs::create_log_file()?;

    // Emit download event only if uv isn't already cached
    if !uv::uv_bin_path().exists() {
        emit_startup_progress("downloading-uv", Some("Downloading runtime tools (~30 MB)"));
    }

    // Try uv first
    match uv::get_or_download_uv().await {
        Ok(uv_path) => {
            println!("[CLAP] Starting server via uv: {:?}", uv_path);
            emit_startup_progress("starting-process", Some("Starting Python server"));
            let child = Command::new(&uv_path)
                .args([
                    "run",
                    "--python",
                    "3.13",
                    "clap_server.py",
                ])
                .current_dir(clap_dir)
                .env("UV_CACHE_DIR", uv::uv_cache_dir())
                .stdout(stdout)
                .stderr(stderr)
                .spawn()
                .map_err(|e| {
                    format!(
                        "Failed to start CLAP server via uv: {}. \
                         Try deleting {:?} and restarting the app. \
                         Log file: {:?}",
                        e,
                        uv::uv_bin_path(),
                        log_path
                    )
                })?;
            Ok(child)
        }
        Err(uv_err) => {
            println!(
                "[CLAP] uv not available ({}), trying manual venv fallback...",
                uv_err
            );
            start_server_venv_fallback(clap_dir, stdout, stderr, log_path)
        }
    }
}

/// Fallback: start server using a manually-created venv.
fn start_server_venv_fallback(
    clap_dir: &std::path::Path,
    stdout: Stdio,
    stderr: Stdio,
    log_path: PathBuf,
) -> Result<Child, String> {
    #[cfg(windows)]
    let python_path = clap_dir.join("venv").join("Scripts").join("python.exe");
    #[cfg(not(windows))]
    let python_path = clap_dir.join("venv").join("bin").join("python");

    if !python_path.exists() {
        return Err(format!(
            "Failed to set up Python environment automatically, and no manual venv found. \
             Check your internet connection and restart the app, or see clap-server/README.md \
             for manual setup instructions. Log file: {:?}",
            log_path
        ));
    }

    println!("[CLAP] Using manual venv fallback: {:?}", python_path);

    Command::new(&python_path)
        .args([
            "-m",
            "uvicorn",
            "clap_server:app",
            "--host",
            "127.0.0.1",
            "--port",
            "5555",
        ])
        .current_dir(clap_dir)
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to start CLAP server: {} (python: {:?}). Log file: {:?}",
                e, python_path, log_path
            )
        })
}

/// Wait for the server to become healthy (up to 120 seconds).
///
/// First run can take a while: uv downloads Python (~25MB) + dependencies (~500MB)
/// + the HuggingFace model (~1-2GB).
async fn wait_for_server_ready() -> Result<(), String> {
    println!("[CLAP] Waiting for server to be ready (up to 120s for first-run setup)...");
    for i in 0..240 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if get_clap_client().await.health_check().await.is_ok() {
            println!("[CLAP] Server ready after {}ms", (i + 1) * 500);
            return Ok(());
        }
    }

    Err(format!(
        "Semantic search server failed to start within 120 seconds. \
         This may happen on first run if the AI model is still downloading. \
         Try restarting the app to resume the download. \
         Check logs in {:?} for details.",
        logs::log_dir()
    ))
}

/// Call the /preload endpoint to ensure the model is loaded.
/// Non-fatal — just logs if it fails.
async fn call_preload() {
    match get_clap_client()
        .await
        .preload()
        .await
    {
        Ok(()) => println!("[CLAP] Model preloaded successfully"),
        Err(e) => println!("[CLAP] Preload call failed (non-fatal): {}", e),
    }
}

/// Stops the CLAP server if we started it
pub fn stop_server() {
    // Use try_lock to avoid blocking - this is called during shutdown
    if let Ok(mut guard) = SERVER_PROCESS.try_lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
}
