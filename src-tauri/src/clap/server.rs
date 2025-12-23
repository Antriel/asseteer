//! CLAP server lifecycle management

use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::sync::Mutex;

use super::client::get_clap_client;

use once_cell::sync::Lazy;

static SERVER_PROCESS: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

/// Ensures CLAP server is running, starts it if needed
pub async fn ensure_server_running() -> Result<(), String> {
    println!("[CLAP] Checking if server is running...");

    // Check if already running (quick check without lock)
    if get_clap_client().await.health_check().await.is_ok() {
        println!("[CLAP] Server already running");
        return Ok(());
    }

    println!("[CLAP] Server not running, attempting to start...");

    // Acquire lock
    let mut guard = SERVER_PROCESS.lock().await;

    // Double-check after acquiring lock
    if get_clap_client().await.health_check().await.is_ok() {
        println!("[CLAP] Server started by another task");
        return Ok(());
    }

    // Get the path to clap-python-prototype relative to project root
    // When running via Tauri, cwd is src-tauri, so go up one level
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current dir: {}", e))?;

    // Try to find clap-python-prototype - it could be in cwd or parent
    let clap_dir = if cwd.join("clap-python-prototype").exists() {
        cwd.join("clap-python-prototype")
    } else if cwd.parent().map(|p| p.join("clap-python-prototype").exists()).unwrap_or(false) {
        cwd.parent().unwrap().join("clap-python-prototype")
    } else {
        return Err(format!("Could not find clap-python-prototype directory (cwd: {:?})", cwd));
    };

    // Use the venv Python executable
    #[cfg(windows)]
    let python_path = clap_dir.join("venv").join("Scripts").join("python.exe");
    #[cfg(not(windows))]
    let python_path = clap_dir.join("venv").join("bin").join("python");

    if !python_path.exists() {
        return Err(format!(
            "Python venv not found at {:?}. Run: cd clap-python-prototype && python -m venv venv && venv\\Scripts\\pip install -r requirements.txt",
            python_path
        ));
    }

    println!("[CLAP] Starting Python server from: {:?}", clap_dir);
    println!("[CLAP] Using Python: {:?}", python_path);

    // Start Python server using venv
    let child = Command::new(&python_path)
        .args([
            "-m",
            "uvicorn",
            "clap_server:app",
            "--host",
            "127.0.0.1",
            "--port",
            "5555",
        ])
        .current_dir(&clap_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start CLAP server: {} (python: {:?})", e, python_path))?;

    println!("[CLAP] Python process spawned with PID: {}", child.id());

    *guard = Some(child);
    drop(guard); // Release lock before waiting

    // Wait for server to be ready (max 60 seconds for model loading on first run)
    println!("[CLAP] Waiting for server to be ready (up to 60s for model download)...");
    for i in 0..120 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if get_clap_client().await.health_check().await.is_ok() {
            println!("[CLAP] Server ready after {}ms", (i + 1) * 500);
            return Ok(());
        }
    }

    println!("[CLAP] Server failed to start within 60 seconds");
    Err("CLAP server failed to start within 60 seconds. Check if the model needs to download.".to_string())
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
