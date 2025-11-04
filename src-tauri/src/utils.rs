use std::fs::File;
use std::io::Read;
use zip::ZipArchive;
use crate::models::Asset;

/// Load asset bytes from either filesystem or zip archive
///
/// If the asset has a `zip_entry`, it will be extracted from the zip archive.
/// Otherwise, it will be read directly from the filesystem.
pub fn load_asset_bytes(asset: &Asset) -> Result<Vec<u8>, String> {
    if let Some(zip_entry_path) = &asset.zip_entry {
        // Asset is inside a zip file
        load_from_zip(&asset.path, zip_entry_path)
    } else {
        // Asset is a regular file
        load_from_filesystem(&asset.path)
    }
}

/// Load bytes from a regular filesystem path
fn load_from_filesystem(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path)
        .map_err(|e| format!("Failed to read file {}: {}", path, e))
}

/// Load bytes from a zip archive entry
fn load_from_zip(zip_path: &str, entry_path: &str) -> Result<Vec<u8>, String> {
    let zip_file = File::open(zip_path)
        .map_err(|e| format!("Failed to open zip file {}: {}", zip_path, e))?;

    let mut archive = ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to read zip archive {}: {}", zip_path, e))?;

    let mut entry = archive.by_name(entry_path)
        .map_err(|e| format!("Failed to find entry {} in zip {}: {}", entry_path, zip_path, e))?;

    let mut buffer = Vec::new();
    entry.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read entry {} from zip {}: {}", entry_path, zip_path, e))?;

    Ok(buffer)
}
