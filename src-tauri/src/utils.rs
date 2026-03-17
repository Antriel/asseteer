use std::fs::File;
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive;
use crate::models::Asset;

/// Load asset bytes from either filesystem or zip archive
///
/// If the asset has a `zip_entry`, it will be extracted from the zip archive.
/// Otherwise, it will be read directly from the filesystem.
pub fn load_asset_bytes(asset: &Asset) -> Result<Vec<u8>, String> {
    if let Some(zip_entry_path) = &asset.zip_entry {
        // Asset is inside a zip file (possibly nested)
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

    // -- load_asset_bytes: filesystem ---------------------------------------

    #[test]
    fn test_load_asset_bytes_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_test_png(dir.path(), "test.png");

        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            path: img_path.to_str().unwrap().into(),
            zip_entry: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            created_at: 0,
            modified_at: 0,
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
            path: "/nonexistent/path/nope.png".into(),
            zip_entry: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            created_at: 0,
            modified_at: 0,
        };

        assert!(load_asset_bytes(&asset).is_err());
    }

    // -- load_asset_bytes: zip entries --------------------------------------

    #[test]
    fn test_load_asset_bytes_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let (zip_path, entry_name) = create_test_zip_with_image(dir.path());

        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            path: zip_path.to_str().unwrap().into(),
            zip_entry: Some(entry_name),
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            created_at: 0,
            modified_at: 0,
        };

        let bytes = load_asset_bytes(&asset).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_load_asset_bytes_zip_bad_entry() {
        let dir = tempfile::tempdir().unwrap();
        let (zip_path, _) = create_test_zip_with_image(dir.path());

        let asset = Asset {
            id: 1,
            filename: "nope.png".into(),
            path: zip_path.to_str().unwrap().into(),
            zip_entry: Some("nonexistent/entry.png".into()),
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            created_at: 0,
            modified_at: 0,
        };

        assert!(load_asset_bytes(&asset).is_err());
    }
}
