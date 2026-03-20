//! CLAP server lifecycle management
//!
//! Starts the CLAP Python server using `uv run` (automatic Python management)
//! with fallback to a manual venv if configured.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::sync::Mutex;

use super::client::{get_clap_client, set_active_port};
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

/// Try ports 5555, 5556, 5557 in sequence and return the first one that is free.
/// Falls back to 5555 if all are in use (server startup will then fail with a clear error).
///
/// Uses a connection attempt rather than bind to detect in-use ports. On Windows,
/// TcpListener::bind succeeds even when another process is already listening (SO_REUSEADDR),
/// so we must try connecting instead — if a connection succeeds, something is listening there.
fn find_free_port() -> u16 {
    for port in [5555u16, 5556, 5557] {
        let in_use = std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            std::time::Duration::from_millis(50),
        )
        .is_ok();
        if !in_use {
            return port;
        }
    }
    println!("[CLAP] Warning: ports 5555-5557 all appear to be in use, trying 5555 anyway");
    5555
}

/// Returns true if we have previously spawned a server process this session.
pub async fn is_server_running() -> bool {
    SERVER_PROCESS.lock().await.is_some()
}

/// Ensures CLAP server is running, starts it if needed
pub async fn ensure_server_running() -> Result<(), String> {
    // Acquire lock first — the process slot is the source of truth.
    // We never probe arbitrary ports to avoid hitting unrelated services.
    let mut guard = SERVER_PROCESS.lock().await;

    if guard.is_some() {
        emit_startup_progress("ready", None);
        return Ok(());
    }

    println!("[CLAP] Server not running, attempting to start...");

    let clap_dir = find_clap_server_dir()?;

    let port = find_free_port();
    println!("[CLAP] Using port {}", port);
    set_active_port(port);

    let (child, log_path) = match start_server_process(&clap_dir, port).await {
        Ok(pair) => pair,
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
    emit_startup_progress("waiting-for-server", Some("Starting Python server…"));
    let child_ref = guard.as_mut().expect("just assigned");
    if let Err(e) = wait_for_server_ready(child_ref, &log_path).await {
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

/// Detect GPU compute capability via nvidia-smi and return the appropriate
/// PyTorch CUDA index URL. Returns None for CPU-only (no GPU or detection fails).
fn detect_pytorch_index() -> Option<String> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=compute_cap", "--format=csv,noheader"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        println!("[CLAP] nvidia-smi failed, using CPU-only PyTorch");
        return None;
    }

    let cap_str = String::from_utf8_lossy(&output.stdout);
    // Parse first GPU's compute capability (e.g. "6.1" or "8.9")
    let major: u32 = cap_str.trim().split('.').next()?.parse().ok()?;

    let index = if major < 7 {
        // Pascal (sm_61) and older: cu126 is the last version with support
        "https://download.pytorch.org/whl/cu126"
    } else {
        // Volta (sm_70) and newer: use latest CUDA
        "https://download.pytorch.org/whl/cu128"
    };

    println!("[CLAP] GPU detected (compute capability {}), using {}", cap_str.trim(), index);
    Some(index.to_string())
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
async fn start_server_process(clap_dir: &std::path::Path, port: u16) -> Result<(Child, PathBuf), String> {
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

            let gpu_index = detect_pytorch_index();
            let port_str = port.to_string();
            let mut args = vec!["run", "--python", "3.13"];
            // Leak the string so we get a &str with the right lifetime for the args vec
            let index_str: Option<&str> = gpu_index.as_ref().map(|s| &**s);
            if let Some(index) = index_str {
                args.extend(["--index", index]);
            }
            args.extend(["clap_server.py", "--port", &port_str]);

            let child = Command::new(&uv_path)
                .args(&args)
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
            Ok((child, log_path))
        }
        Err(uv_err) => {
            println!(
                "[CLAP] uv not available ({}), trying manual venv fallback...",
                uv_err
            );
            let child = start_server_venv_fallback(clap_dir, port, stdout, stderr, log_path.clone())?;
            Ok((child, log_path))
        }
    }
}

/// Fallback: start server using a manually-created venv.
fn start_server_venv_fallback(
    clap_dir: &std::path::Path,
    port: u16,
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
    let port_str = port.to_string();

    Command::new(&python_path)
        .args([
            "-m",
            "uvicorn",
            "clap_server:app",
            "--host",
            "127.0.0.1",
            "--port",
            &port_str,
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

/// Wait for the server to become healthy (up to 30 minutes).
///
/// First run can take a while: uv downloads Python (~25MB) + dependencies
/// (~500MB CPU / ~8GB GPU) + the HuggingFace model (~1-2GB).
/// We check process liveness on every iteration for a fast-fail if the
/// process died, and tail the log every 10s to show download progress.
async fn wait_for_server_ready(child: &mut Child, log_path: &std::path::Path) -> Result<(), String> {
    println!("[CLAP] Waiting for server to be ready (GPU first-run may take 20+ minutes)...");

    // 30 minutes at 500ms intervals
    for i in 0..3600u32 {
        // Fast-fail: if the child process has exited, no point waiting further
        match child.try_wait() {
            Ok(Some(status)) => {
                let tail = read_log_tail(log_path).unwrap_or_default();
                let tail_section = if tail.is_empty() {
                    String::new()
                } else {
                    format!("\n\nLast log output:\n{}", tail)
                };
                return Err(format!(
                    "CLAP server process exited unexpectedly (exit code: {}).{}\n\nCheck logs in {:?} for details.",
                    status, tail_section, logs::log_dir()
                ));
            }
            _ => {} // still running or check failed — keep waiting
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        if get_clap_client().await.health_check().await.is_ok() {
            println!("[CLAP] Server ready after {}ms", (i + 1) * 500);
            return Ok(());
        }

        // Every 10 seconds, tail the log and surface the last meaningful line
        // as the startup detail so the UI shows real download progress.
        if i > 0 && i % 20 == 19 {
            if let Some(line) = read_log_last_meaningful_line(log_path) {
                emit_startup_progress("waiting-for-server", Some(&line));
            }
        }
    }

    Err(format!(
        "Semantic search server is still not responding after 30 minutes. \
         On first run with GPU support, PyTorch CUDA (~8 GB) may still be downloading — \
         keep the app open rather than restarting, as restarting will cancel the download. \
         Check logs in {:?} for details.",
        logs::log_dir()
    ))
}

/// Read the last ~4 KB of the log file and return the last line that looks
/// like meaningful uv/pip output (download progress, installs, errors).
/// Falls back to the last non-empty line if nothing keyword-matched.
fn read_log_last_meaningful_line(log_path: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(log_path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(4096))).ok()?;

    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;

    let keywords = ["Downloading", "Installing", "Resolved", "Built", "Fetching", "Audited", "error", "Error"];

    // Prefer a line that looks like uv/pip progress
    let meaningful = buf.lines().rev().find(|l| {
        let trimmed = l.trim();
        !trimmed.is_empty() && keywords.iter().any(|kw| trimmed.contains(kw))
    });

    let line = meaningful
        .or_else(|| buf.lines().rev().find(|l| !l.trim().is_empty()))?
        .trim();

    // Truncate long lines (e.g. full wheel URLs)
    Some(if line.len() > 80 {
        format!("{}…", &line[..80])
    } else {
        line.to_string()
    })
}

/// Read the last ~2 KB of the log file as a plain string (for error reports).
fn read_log_tail(log_path: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(log_path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(2048))).ok()?;

    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    Some(buf.trim().to_string())
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

/// Stops the CLAP server and waits for the process to fully exit.
/// Use this before deleting files the server may have open.
pub async fn stop_server_and_wait() {
    let mut guard = SERVER_PROCESS.lock().await;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        // Wait for the process to exit so the OS releases file handles
        let _ = child.wait();
    }
}
