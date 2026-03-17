//! `uv` binary download and management
//!
//! Downloads and caches the Astral `uv` binary for managing the Python
//! environment used by the CLAP server. On first use, downloads `uv` from
//! GitHub releases (~30MB), then uses it to run the CLAP server with
//! automatic Python installation and dependency resolution.

use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;

/// Pinned uv version (0.6.x range)
const UV_VERSION: &str = "0.6.14";

/// Global app data directory, set once at startup
static APP_DATA_DIR: OnceCell<PathBuf> = OnceCell::new();

/// Initialize the app data directory. Must be called once during app setup.
pub fn init_app_data_dir(dir: PathBuf) {
    let _ = APP_DATA_DIR.set(dir);
}

/// Get the app data directory, or fall back to a temp directory.
pub(super) fn app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::temp_dir().join("asseteer"))
}

/// Get the expected path to the uv binary.
pub fn uv_bin_path() -> PathBuf {
    let uv_dir = app_data_dir().join("uv");
    if cfg!(windows) {
        uv_dir.join("uv.exe")
    } else {
        uv_dir.join("uv")
    }
}

/// Get the uv cache directory (where Python and packages are stored).
pub fn uv_cache_dir() -> PathBuf {
    app_data_dir().join("uv-cache")
}

/// Returns the path to the uv binary, downloading it if not present.
pub async fn get_or_download_uv() -> Result<PathBuf, String> {
    let uv_path = uv_bin_path();

    if uv_path.exists() {
        println!("[UV] Found cached uv at {:?}", uv_path);
        return Ok(uv_path);
    }

    println!("[UV] uv not found, downloading v{}...", UV_VERSION);

    let url = download_url();
    println!("[UV] Downloading from {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to download uv: {}. Check your internet connection.", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download uv: HTTP {}. URL: {}",
            response.status(),
            url
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read uv download: {}", e))?;

    println!("[UV] Downloaded {} bytes, extracting...", bytes.len());

    let uv_dir = app_data_dir().join("uv");
    std::fs::create_dir_all(&uv_dir)
        .map_err(|e| format!("Failed to create uv directory: {}", e))?;

    extract_uv_binary(&bytes, &uv_dir)?;

    if !uv_path.exists() {
        return Err(format!(
            "uv binary not found after extraction at {:?}",
            uv_path
        ));
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&uv_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set uv permissions: {}", e))?;
    }

    println!("[UV] uv v{} installed at {:?}", UV_VERSION, uv_path);
    Ok(uv_path)
}

/// Build the platform-specific download URL for the uv release.
fn download_url() -> String {
    let (arch, platform, ext) = platform_triple();
    format!(
        "https://github.com/astral-sh/uv/releases/download/{}/uv-{}-{}.{}",
        UV_VERSION, arch, platform, ext
    )
}

/// Returns (arch, platform, extension) for the current build target.
fn platform_triple() -> (&'static str, &'static str, &'static str) {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        ("x86_64", "pc-windows-msvc", "zip")
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        ("aarch64", "apple-darwin", "tar.gz")
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        ("x86_64", "apple-darwin", "tar.gz")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        ("x86_64", "unknown-linux-gnu", "tar.gz")
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        ("aarch64", "unknown-linux-gnu", "tar.gz")
    }
}

/// Extract the uv binary from the downloaded archive into `dest_dir`.
fn extract_uv_binary(archive_bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    let (_, _, ext) = platform_triple();
    match ext {
        "zip" => extract_from_zip(archive_bytes, dest_dir),
        "tar.gz" => extract_from_tar_gz(archive_bytes, dest_dir),
        _ => Err(format!("Unsupported archive format: {}", ext)),
    }
}

/// Extract uv binary from a .zip archive (Windows).
fn extract_from_zip(bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Failed to open zip archive: {}", e))?;

    let uv_name = if cfg!(windows) { "uv.exe" } else { "uv" };

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let name = file.name().to_string();
        // The zip contains entries like "uv-x86_64-pc-windows-msvc/uv.exe"
        if name.ends_with(uv_name) && !name.contains("uvx") {
            let dest_path = dest_dir.join(uv_name);
            let mut out = std::fs::File::create(&dest_path)
                .map_err(|e| format!("Failed to create {}: {}", uv_name, e))?;
            std::io::copy(&mut file, &mut out)
                .map_err(|e| format!("Failed to write {}: {}", uv_name, e))?;
            println!("[UV] Extracted {} from {}", uv_name, name);
            return Ok(());
        }
    }

    Err(format!("{} not found in zip archive", uv_name))
}

/// Extract uv binary from a .tar.gz archive (macOS/Linux).
fn extract_from_tar_gz(bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = Archive::new(gz);

    let entries = archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {}", e))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {}", e))?;
        let path_str = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?
            .to_path_buf();

        let file_name = path_str
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Extract only the uv binary (not uvx)
        if file_name == "uv" {
            let dest_path = dest_dir.join("uv");
            entry
                .unpack(&dest_path)
                .map_err(|e| format!("Failed to extract uv: {}", e))?;
            println!("[UV] Extracted uv from {:?}", path_str);
            return Ok(());
        }
    }

    Err("uv binary not found in tar archive".to_string())
}
