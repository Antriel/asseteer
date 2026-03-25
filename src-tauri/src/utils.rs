use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::time::UNIX_EPOCH;
use zip::ZipArchive;
use crate::models::Asset;

/// Seconds since Unix epoch as i64.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Milliseconds since Unix epoch as u64.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Resolve the absolute filesystem path for a regular (non-ZIP) asset.
/// Returns: folder_path/rel_path/filename (or folder_path/filename if rel_path is empty)
pub fn resolve_asset_fs_path(asset: &Asset) -> String {
    if asset.rel_path.is_empty() {
        format!("{}/{}", asset.folder_path, asset.filename)
    } else {
        format!("{}/{}/{}", asset.folder_path, asset.rel_path, asset.filename)
    }
}

/// Resolve the absolute filesystem path to the ZIP archive containing this asset.
/// Returns: folder_path/rel_path/zip_file (or folder_path/zip_file if rel_path is empty)
pub fn resolve_zip_path(asset: &Asset) -> String {
    let zip_file = asset.zip_file.as_deref().unwrap_or("");
    if asset.rel_path.is_empty() {
        format!("{}/{}", asset.folder_path, zip_file)
    } else {
        format!("{}/{}/{}", asset.folder_path, asset.rel_path, zip_file)
    }
}

/// Load asset bytes from either filesystem or zip archive.
///
/// The asset must have `folder_path` populated (via JOIN with source_folders).
/// If the asset has a `zip_entry`, it will be extracted from the zip archive.
/// Otherwise, it will be read directly from the filesystem.
pub fn load_asset_bytes(asset: &Asset) -> Result<Vec<u8>, String> {
    if let Some(zip_entry_path) = &asset.zip_entry {
        let zip_path = resolve_zip_path(asset);
        load_from_zip(&zip_path, zip_entry_path)
    } else {
        let fs_path = resolve_asset_fs_path(asset);
        load_from_filesystem(&fs_path)
    }
}

/// Load bytes from a regular filesystem path
fn load_from_filesystem(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path)
        .map_err(|e| format!("Failed to read file {}: {}", path, e))
}

/// Load bytes from a zip archive entry (supports nested zips)
///
/// The entry_path can contain nested zips, e.g., "inner.zip/folder/image.png"
/// which means: open inner.zip inside the outer zip, then extract folder/image.png
fn load_from_zip(zip_path: &str, entry_path: &str) -> Result<Vec<u8>, String> {
    let zip_file = File::open(zip_path)
        .map_err(|e| format!("Failed to open zip file {}: {}", zip_path, e))?;

    let archive = ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to read zip archive {}: {}", zip_path, e))?;

    load_from_zip_recursive(archive, entry_path, zip_path)
}

/// Recursively extract from potentially nested zip archives
fn load_from_zip_recursive<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    entry_path: &str,
    context: &str,  // For error messages
) -> Result<Vec<u8>, String> {
    // Check if entry_path contains a nested zip
    // We look for ".zip/" pattern which indicates a nested zip to traverse
    if let Some(zip_boundary) = find_nested_zip_boundary(entry_path) {
        let (nested_zip_path, remaining_path) = entry_path.split_at(zip_boundary);
        let remaining_path = &remaining_path[1..]; // Skip the '/'

        // Extract the nested zip into memory
        let mut entry = archive.by_name(nested_zip_path)
            .map_err(|e| format!("Failed to find nested zip {} in {}: {}", nested_zip_path, context, e))?;

        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read nested zip {} from {}: {}", nested_zip_path, context, e))?;

        // Open the nested zip and continue recursively
        let cursor = Cursor::new(buffer);
        let nested_archive = ZipArchive::new(cursor)
            .map_err(|e| format!("Failed to open nested zip {} from {}: {}", nested_zip_path, context, e))?;

        let nested_context = format!("{}/{}", context, nested_zip_path);
        load_from_zip_recursive(nested_archive, remaining_path, &nested_context)
    } else {
        // No more nested zips, extract the final file
        let mut entry = archive.by_name(entry_path)
            .map_err(|e| format!("Failed to find entry {} in {}: {}", entry_path, context, e))?;

        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read entry {} from {}: {}", entry_path, context, e))?;

        Ok(buffer)
    }
}

/// Find the boundary index where a nested zip ends (position after ".zip")
/// Returns None if no nested zip is found in the path
///
/// Example: "inner.zip/folder/image.png" -> Some(9) (position after "inner.zip")
/// Example: "folder/image.png" -> None
fn find_nested_zip_boundary(path: &str) -> Option<usize> {
    // Look for ".zip/" pattern (case-insensitive for the extension)
    let path_lower = path.to_lowercase();

    path_lower.find(".zip/").map(|pos| pos + 4) // Position after ".zip"
}

/// Load multiple entries from the same ZIP archive in one pass.
/// Opens the archive once and extracts all requested entries, avoiding
/// repeated central-directory parsing that dominates per-entry I/O cost.
///
/// Returns a map from asset_id to the extracted bytes (or an error string).
pub fn bulk_load_from_zip(zip_path: &str, entries: &[(i64, &str)]) -> HashMap<i64, Result<Vec<u8>, String>> {
    let mut results = HashMap::with_capacity(entries.len());

    let zip_file = match File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            let err = format!("Failed to open zip file {}: {}", zip_path, e);
            for &(id, _) in entries {
                results.insert(id, Err(err.clone()));
            }
            return results;
        }
    };

    let mut archive = match ZipArchive::new(std::io::BufReader::new(zip_file)) {
        Ok(a) => a,
        Err(e) => {
            let err = format!("Failed to read zip archive {}: {}", zip_path, e);
            for &(id, _) in entries {
                results.insert(id, Err(err.clone()));
            }
            return results;
        }
    };

    for &(id, entry_path) in entries {
        let result = match archive.by_name(entry_path) {
            Ok(mut entry) => {
                let mut buffer = Vec::with_capacity(entry.size() as usize);
                match entry.read_to_end(&mut buffer) {
                    Ok(_) => Ok(buffer),
                    Err(e) => Err(format!(
                        "Failed to read entry {} from {}: {}",
                        entry_path, zip_path, e
                    )),
                }
            }
            Err(e) => Err(format!(
                "Failed to find entry {} in {}: {}",
                entry_path, zip_path, e
            )),
        };
        results.insert(id, result);
    }

    results
}

/// Extract entries for a batch of assets from an already-opened ZIP archive.
/// Avoids re-parsing the central directory for each batch within a group.
pub fn bulk_load_from_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    assets: &[Asset],
) -> HashMap<i64, Result<Vec<u8>, String>> {
    let mut results = HashMap::with_capacity(assets.len());
    for asset in assets {
        if let Some(entry_path) = &asset.zip_entry {
            let result = match archive.by_name(entry_path) {
                Ok(mut entry) => {
                    let mut buffer = Vec::with_capacity(entry.size() as usize);
                    match entry.read_to_end(&mut buffer) {
                        Ok(_) => Ok(buffer),
                        Err(e) => Err(format!(
                            "Failed to read entry {} from archive: {}",
                            entry_path, e
                        )),
                    }
                }
                Err(e) => Err(format!("Failed to find entry {} in archive: {}", entry_path, e)),
            };
            results.insert(asset.id, result);
        }
    }
    results
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    // -- find_nested_zip_boundary -------------------------------------------

    #[test]
    fn test_zip_boundary_simple() {
        assert_eq!(
            find_nested_zip_boundary("inner.zip/folder/image.png"),
            Some(9)
        );
    }

    #[test]
    fn test_zip_boundary_no_zip() {
        assert_eq!(find_nested_zip_boundary("folder/image.png"), None);
    }

    #[test]
    fn test_zip_boundary_zip_at_end_no_slash() {
        assert_eq!(find_nested_zip_boundary("archive.zip"), None);
    }

    #[test]
    fn test_zip_boundary_case_insensitive() {
        assert_eq!(
            find_nested_zip_boundary("ARCHIVE.ZIP/image.png"),
            Some(11)
        );
    }

    #[test]
    fn test_zip_boundary_deeply_nested() {
        // First .zip/ boundary wins
        assert_eq!(
            find_nested_zip_boundary("outer.zip/inner.zip/file.png"),
            Some(9)
        );
    }

    // -- resolve paths ------------------------------------------------------

    #[test]
    fn test_resolve_asset_fs_path_with_rel_path() {
        let asset = Asset {
            id: 1,
            filename: "grass.png".into(),
            folder_id: 1,
            rel_path: "Packs/textures".into(),
            zip_file: None,
            zip_entry: None,
            zip_compression: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "D:/Assets".into(),
        };
        assert_eq!(resolve_asset_fs_path(&asset), "D:/Assets/Packs/textures/grass.png");
    }

    #[test]
    fn test_resolve_asset_fs_path_root() {
        let asset = Asset {
            id: 1,
            filename: "grass.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: None,
            zip_entry: None,
            zip_compression: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "D:/Assets".into(),
        };
        assert_eq!(resolve_asset_fs_path(&asset), "D:/Assets/grass.png");
    }

    #[test]
    fn test_resolve_zip_path() {
        let asset = Asset {
            id: 1,
            filename: "forest_01.wav".into(),
            folder_id: 1,
            rel_path: "Packs".into(),
            zip_file: Some("sounds.zip".into()),
            zip_entry: Some("ambient/forest_01.wav".into()),
            zip_compression: None,
            asset_type: "audio".into(),
            format: "wav".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "D:/Assets".into(),
        };
        assert_eq!(resolve_zip_path(&asset), "D:/Assets/Packs/sounds.zip");
    }

    // -- load_asset_bytes: filesystem ---------------------------------------

    #[test]
    fn test_load_asset_bytes_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        create_test_png(dir.path(), "test.png");
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");

        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: None,
            zip_entry: None,
            zip_compression: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path,
        };

        let bytes = load_asset_bytes(&asset).unwrap();
        assert!(!bytes.is_empty());
        // PNG magic bytes
        assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_load_asset_bytes_nonexistent_file() {
        let asset = Asset {
            id: 1,
            filename: "nope.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: None,
            zip_entry: None,
            zip_compression: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "/nonexistent/path".into(),
        };

        assert!(load_asset_bytes(&asset).is_err());
    }

    // -- load_asset_bytes: zip entries --------------------------------------

    #[test]
    fn test_load_asset_bytes_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let (_zip_path, entry_name) = create_test_zip_with_image(dir.path());
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");

        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: Some("test_archive.zip".into()),
            zip_entry: Some(entry_name),
            zip_compression: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path,
        };

        let bytes = load_asset_bytes(&asset).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_load_asset_bytes_zip_bad_entry() {
        let dir = tempfile::tempdir().unwrap();
        let (_zip_path, _) = create_test_zip_with_image(dir.path());
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");

        let asset = Asset {
            id: 1,
            filename: "nope.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: Some("test_archive.zip".into()),
            zip_entry: Some("nonexistent/entry.png".into()),
            zip_compression: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path,
        };

        assert!(load_asset_bytes(&asset).is_err());
    }
}
