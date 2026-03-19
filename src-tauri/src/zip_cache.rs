//! Global cache for nested ZIP archives.
//!
//! Holds at most one decompressed inner ZIP in memory at a time.
//! A `Mutex` serializes access so that only one thread can decompress
//! or read from a nested ZIP concurrently, preventing OOM when many
//! workers would otherwise each decompress the same large inner ZIP.

use crate::models::Asset;
use crate::utils::{load_asset_bytes, resolve_zip_path};
use std::io::{Cursor, Read};
use std::sync::Mutex;
use zip::ZipArchive;

struct CachedInnerZip {
    /// Resolved path to the outer ZIP on disk.
    outer_path: String,
    /// Entry name of the inner ZIP within the outer (e.g. "inner.zip").
    inner_name: String,
    /// The decompressed inner ZIP archive, held in memory.
    archive: ZipArchive<Cursor<Vec<u8>>>,
}

static CACHE: Mutex<Option<CachedInnerZip>> = Mutex::new(None);

/// Load asset bytes, using a shared cache for nested ZIP archives.
///
/// - Non-ZIP assets: reads directly from filesystem (no locking).
/// - Simple ZIP assets (not nested): reads directly from the outer ZIP.
/// - Nested ZIP assets: acquires the global cache mutex, checks/populates
///   the cached inner ZIP, and extracts the file. At most one inner ZIP
///   is decompressed at a time.
pub fn load_asset_bytes_cached(asset: &Asset) -> Result<Vec<u8>, String> {
    let zip_entry_path = match &asset.zip_entry {
        Some(p) => p,
        None => return load_asset_bytes(asset),
    };

    // Check if this is a nested ZIP (has .zip/ in the entry path)
    let path_lower = zip_entry_path.to_lowercase();
    let nested_boundary = path_lower.find(".zip/").map(|pos| pos + 4);

    let boundary = match nested_boundary {
        Some(b) => b,
        // Not nested — use regular (uncached) loading
        None => return load_asset_bytes(asset),
    };

    let inner_zip_name = &zip_entry_path[..boundary];
    let inner_entry = &zip_entry_path[boundary + 1..]; // skip the '/'
    let outer_zip_path = resolve_zip_path(asset);

    // Lock the cache — serializes all nested ZIP access
    let mut guard = CACHE.lock().map_err(|e| format!("Cache lock poisoned: {}", e))?;

    // Check if the right inner ZIP is already cached
    let cache_hit = guard
        .as_ref()
        .map(|c| c.outer_path == outer_zip_path && c.inner_name == inner_zip_name)
        .unwrap_or(false);

    if !cache_hit {
        // Read and decompress the inner ZIP from the outer
        let zip_file = std::fs::File::open(&outer_zip_path)
            .map_err(|e| format!("Failed to open zip {}: {}", outer_zip_path, e))?;
        let mut outer = ZipArchive::new(zip_file)
            .map_err(|e| format!("Failed to read zip {}: {}", outer_zip_path, e))?;

        let mut inner_bytes = Vec::new();
        {
            let mut entry = outer.by_name(inner_zip_name).map_err(|e| {
                format!(
                    "Failed to find {} in {}: {}",
                    inner_zip_name, outer_zip_path, e
                )
            })?;
            entry.read_to_end(&mut inner_bytes).map_err(|e| {
                format!(
                    "Failed to read {} from {}: {}",
                    inner_zip_name, outer_zip_path, e
                )
            })?;
        }

        let cursor = Cursor::new(inner_bytes);
        let inner_archive = ZipArchive::new(cursor)
            .map_err(|e| format!("Failed to open inner zip {}: {}", inner_zip_name, e))?;

        *guard = Some(CachedInnerZip {
            outer_path: outer_zip_path,
            inner_name: inner_zip_name.to_string(),
            archive: inner_archive,
        });
    }

    // Extract the file from the cached inner ZIP
    let cached = guard.as_mut().unwrap();
    let mut entry = cached
        .archive
        .by_name(inner_entry)
        .map_err(|e| format!("Failed to find {} in inner zip: {}", inner_entry, e))?;
    let mut buffer = Vec::new();
    entry
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read {} from inner zip: {}", inner_entry, e))?;
    Ok(buffer)
}

/// Clear the cache. Call when processing is done or stopped to free memory.
pub fn clear() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_zip_asset_bypasses_cache() {
        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: None,
            zip_entry: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "/nonexistent".into(),
        };
        // Should attempt filesystem read (and fail), not panic on cache
        assert!(load_asset_bytes_cached(&asset).is_err());
    }

    #[test]
    fn test_simple_zip_asset_bypasses_cache() {
        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: Some("archive.zip".into()),
            zip_entry: Some("folder/test.png".into()), // no nested .zip/
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "/nonexistent".into(),
        };
        // Should attempt regular zip read (and fail), not use cache
        assert!(load_asset_bytes_cached(&asset).is_err());
    }

    #[test]
    fn test_clear_does_not_panic() {
        clear();
    }
}
