//! Global cache for nested ZIP archives.
//!
//! Holds one decompressed nested ZIP as shared immutable bytes.
//! A short mutex/condvar section coordinates cache population so that
//! only one thread loads a given nested ZIP at a time, while cache hits
//! can then be read in parallel by opening fresh `ZipArchive`s over the
//! shared bytes.

use crate::models::Asset;
use crate::utils::{load_asset_bytes, resolve_zip_path};
use std::io::{Cursor, Read, Seek};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
use zip::ZipArchive;

struct CachedInnerZip {
    key: String,
    bytes: Arc<Vec<u8>>,
}

#[derive(Default)]
struct ActiveKeyState {
    active_key: Option<String>,
    active_users: usize,
}

enum CacheState {
    Empty,
    Loading(String),
    Ready(CachedInnerZip),
}

static CACHE: Mutex<CacheState> = Mutex::new(CacheState::Empty);
static CACHE_READY: Condvar = Condvar::new();
static ACTIVE_KEY: Mutex<ActiveKeyState> = Mutex::new(ActiveKeyState {
    active_key: None,
    active_users: 0,
});
static ACTIVE_KEY_READY: Condvar = Condvar::new();

struct ActiveKeyGuard {
    key: String,
}

/// Load asset bytes, using a shared cache for nested ZIP archives.
///
/// - Non-ZIP assets: reads directly from filesystem (no locking).
/// - Simple ZIP assets (not nested): reads directly from the outer ZIP.
/// - Nested ZIP assets: caches the deepest nested ZIP bytes as `Arc<Vec<u8>>`
///   and opens a fresh `ZipArchive` per caller, allowing parallel reads from
///   the same nested ZIP without duplicating the decompressed archive in memory.
pub fn load_asset_bytes_cached(asset: &Asset) -> Result<Vec<u8>, String> {
    let zip_entry_path = match &asset.zip_entry {
        Some(path) => path,
        None => return load_asset_bytes(asset),
    };

    let Some((nested_zip_path, inner_entry)) = split_deepest_nested_zip_path(zip_entry_path) else {
        return load_asset_bytes(asset);
    };

    let outer_zip_path = resolve_zip_path(asset);
    let key = compose_cache_key(&outer_zip_path, nested_zip_path);
    let _active_key_guard = acquire_active_nested_zip_key(&key)?;
    let cached_zip = get_cached_nested_zip_bytes(&outer_zip_path, nested_zip_path)?;
    load_entry_from_shared_zip_bytes(&cached_zip, inner_entry, nested_zip_path)
}

/// Group key used by scheduling to keep adjacent files from the same nested ZIP together.
pub fn nested_zip_group_key(asset: &Asset) -> Option<String> {
    let zip_entry_path = asset.zip_entry.as_deref()?;
    let (nested_zip_path, _) = split_deepest_nested_zip_path(zip_entry_path)?;
    Some(compose_cache_key(&resolve_zip_path(asset), nested_zip_path))
}

pub fn is_nested_zip_asset(asset: &Asset) -> bool {
    nested_zip_group_key(asset).is_some()
}

fn acquire_active_nested_zip_key(key: &str) -> Result<ActiveKeyGuard, String> {
    let wait_started = Instant::now();
    let mut guard = ACTIVE_KEY
        .lock()
        .map_err(|e| format!("Active-key lock poisoned: {}", e))?;

    loop {
        match &guard.active_key {
            Some(active_key) if active_key == key => {
                guard.active_users += 1;
                println!(
                    "[ZipGate] JOIN key='{}' waited_ms={} active_users={}",
                    key,
                    wait_started.elapsed().as_millis(),
                    guard.active_users
                );
                return Ok(ActiveKeyGuard {
                    key: key.to_string(),
                });
            }
            Some(active_key) => {
                println!(
                    "[ZipGate] WAIT key='{}' active='{}' waited_ms={}",
                    key,
                    active_key,
                    wait_started.elapsed().as_millis()
                );
                guard = ACTIVE_KEY_READY
                    .wait(guard)
                    .map_err(|e| format!("Active-key lock poisoned: {}", e))?;
            }
            None => {
                guard.active_key = Some(key.to_string());
                guard.active_users = 1;
                println!(
                    "[ZipGate] ACTIVATE key='{}' waited_ms={}",
                    key,
                    wait_started.elapsed().as_millis()
                );
                return Ok(ActiveKeyGuard {
                    key: key.to_string(),
                });
            }
        }
    }
}

fn get_cached_nested_zip_bytes(
    outer_zip_path: &str,
    nested_zip_path: &str,
) -> Result<Arc<Vec<u8>>, String> {
    let key = compose_cache_key(outer_zip_path, nested_zip_path);
    let wait_started = Instant::now();

    loop {
        let mut guard = CACHE
            .lock()
            .map_err(|e| format!("Cache lock poisoned: {}", e))?;

        match &*guard {
            CacheState::Ready(cached) if cached.key == key => {
                let waited = wait_started.elapsed();
                println!(
                    "[ZipCache] HIT key='{}' waited_ms={} size_mb={:.1}",
                    key,
                    waited.as_millis(),
                    cached.bytes.len() as f64 / (1024.0 * 1024.0)
                );
                return Ok(cached.bytes.clone());
            }
            CacheState::Loading(loading_key) if loading_key == &key => {
                println!(
                    "[ZipCache] WAIT same key='{}' waited_ms={}",
                    key,
                    wait_started.elapsed().as_millis()
                );
                guard = CACHE_READY
                    .wait(guard)
                    .map_err(|e| format!("Cache lock poisoned: {}", e))?;
                drop(guard);
            }
            CacheState::Loading(_) => {
                println!(
                    "[ZipCache] WAIT other key='{}' waited_ms={}",
                    key,
                    wait_started.elapsed().as_millis()
                );
                guard = CACHE_READY
                    .wait(guard)
                    .map_err(|e| format!("Cache lock poisoned: {}", e))?;
                drop(guard);
            }
            CacheState::Empty | CacheState::Ready(_) => {
                println!(
                    "[ZipCache] LOAD START key='{}' waited_ms={}",
                    key,
                    wait_started.elapsed().as_millis()
                );
                *guard = CacheState::Loading(key.clone());
                drop(guard);
                break;
            }
        }
    }

    let load_started = Instant::now();
    let load_result = load_nested_zip_bytes_uncached(outer_zip_path, nested_zip_path);
    let mut guard = CACHE
        .lock()
        .map_err(|e| format!("Cache lock poisoned: {}", e))?;

    match load_result {
        Ok(bytes) => {
            let shared = Arc::new(bytes);
            println!(
                "[ZipCache] LOAD DONE key='{}' load_ms={} size_mb={:.1}",
                key,
                load_started.elapsed().as_millis(),
                shared.len() as f64 / (1024.0 * 1024.0)
            );
            *guard = CacheState::Ready(CachedInnerZip {
                key,
                bytes: shared.clone(),
            });
            CACHE_READY.notify_all();
            Ok(shared)
        }
        Err(err) => {
            *guard = CacheState::Empty;
            CACHE_READY.notify_all();
            Err(err)
        }
    }
}

fn load_nested_zip_bytes_uncached(
    outer_zip_path: &str,
    nested_zip_path: &str,
) -> Result<Vec<u8>, String> {
    let zip_file = std::fs::File::open(outer_zip_path)
        .map_err(|e| format!("Failed to open zip {}: {}", outer_zip_path, e))?;
    let outer_archive = ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to read zip {}: {}", outer_zip_path, e))?;

    load_nested_zip_bytes_from_archive(outer_archive, nested_zip_path, outer_zip_path)
}

fn load_nested_zip_bytes_from_archive<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    nested_zip_path: &str,
    context: &str,
) -> Result<Vec<u8>, String> {
    if let Some((nested_zip_name, remaining_path)) = split_first_nested_zip_segment(nested_zip_path) {
        let nested_bytes = read_entry_bytes(&mut archive, nested_zip_name, context)?;
        let nested_context = format!("{}/{}", context, nested_zip_name);
        let nested_archive = ZipArchive::new(Cursor::new(nested_bytes))
            .map_err(|e| format!("Failed to open nested zip {}: {}", nested_context, e))?;
        load_nested_zip_bytes_from_archive(nested_archive, remaining_path, &nested_context)
    } else {
        read_entry_bytes(&mut archive, nested_zip_path, context)
            .map_err(|e| format!("Failed to read nested zip {} from {}: {}", nested_zip_path, context, e))
    }
}

fn read_entry_bytes<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    entry_name: &str,
    context: &str,
) -> Result<Vec<u8>, String> {
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| format!("Failed to find {} in {}: {}", entry_name, context, e))?;

    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read {} from {}: {}", entry_name, context, e))?;
    Ok(bytes)
}

fn load_entry_from_shared_zip_bytes(
    zip_bytes: &Arc<Vec<u8>>,
    inner_entry: &str,
    nested_zip_path: &str,
) -> Result<Vec<u8>, String> {
    let archive = ZipArchive::new(Cursor::new(zip_bytes.as_slice()))
        .map_err(|e| format!("Failed to open cached nested zip {}: {}", nested_zip_path, e))?;
    load_entry_from_archive(archive, inner_entry, nested_zip_path)
}

fn load_entry_from_archive<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    inner_entry: &str,
    context: &str,
) -> Result<Vec<u8>, String> {
    let mut entry = archive
        .by_name(inner_entry)
        .map_err(|e| format!("Failed to find {} in {}: {}", inner_entry, context, e))?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read {} from {}: {}", inner_entry, context, e))?;
    Ok(bytes)
}

fn split_deepest_nested_zip_path(path: &str) -> Option<(&str, &str)> {
    let boundary = find_last_nested_zip_boundary(path)?;
    Some((&path[..boundary], &path[boundary + 1..]))
}

fn split_first_nested_zip_segment(path: &str) -> Option<(&str, &str)> {
    let boundary = find_first_nested_zip_boundary(path)?;
    Some((&path[..boundary], &path[boundary + 1..]))
}

fn find_first_nested_zip_boundary(path: &str) -> Option<usize> {
    path.to_ascii_lowercase().find(".zip/").map(|pos| pos + 4)
}

fn find_last_nested_zip_boundary(path: &str) -> Option<usize> {
    path.to_ascii_lowercase().rfind(".zip/").map(|pos| pos + 4)
}

fn compose_cache_key(outer_zip_path: &str, nested_zip_path: &str) -> String {
    format!("{}::{}", outer_zip_path, nested_zip_path)
}

impl Drop for ActiveKeyGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = ACTIVE_KEY.lock() {
            if guard.active_key.as_deref() == Some(self.key.as_str()) {
                guard.active_users = guard.active_users.saturating_sub(1);
                if guard.active_users == 0 {
                    println!("[ZipGate] RELEASE key='{}'", self.key);
                    guard.active_key = None;
                    ACTIVE_KEY_READY.notify_all();
                } else {
                    println!(
                        "[ZipGate] LEAVE key='{}' remaining_users={}",
                        self.key,
                        guard.active_users
                    );
                }
            }
        }
    }
}

/// Clear the cache. Call when processing is done or stopped to free memory.
pub fn clear() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = CacheState::Empty;
        CACHE_READY.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::io::Write;

    fn make_nested_zip_asset(
        folder_path: &str,
        zip_file: &str,
        zip_entry: &str,
        filename: &str,
        format: &str,
    ) -> Asset {
        Asset {
            id: 1,
            filename: filename.into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: Some(zip_file.into()),
            zip_entry: Some(zip_entry.into()),
            asset_type: "audio".into(),
            format: format.into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: folder_path.into(),
        }
    }

    fn create_deep_nested_zip_with_wav(dir: &std::path::Path) -> (String, String, String) {
        let wav_path = create_test_wav(dir, "inner.wav");
        let wav_bytes = std::fs::read(wav_path).unwrap();

        let level2_name = "level2.zip";
        let level1_name = "level1.zip";
        let final_entry = "audio/test.wav";

        let mut level2_bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut level2_bytes);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(final_entry, options).unwrap();
            zip.write_all(&wav_bytes).unwrap();
            zip.finish().unwrap();
        }

        let mut level1_bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut level1_bytes);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(level2_name, options).unwrap();
            zip.write_all(&level2_bytes).unwrap();
            zip.finish().unwrap();
        }

        let outer_zip_path = dir.join("outer.zip");
        {
            let file = std::fs::File::create(&outer_zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(level1_name, options).unwrap();
            zip.write_all(&level1_bytes).unwrap();
            zip.finish().unwrap();
        }

        (
            outer_zip_path.file_name().unwrap().to_string_lossy().into_owned(),
            format!("{}/{}/{}", level1_name, level2_name, final_entry),
            "test.wav".to_string(),
        )
    }

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
            zip_entry: Some("folder/test.png".into()),
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path: "/nonexistent".into(),
        };
        assert!(load_asset_bytes_cached(&asset).is_err());
    }

    #[test]
    fn test_nested_zip_group_key_uses_outer_zip_and_nested_path() {
        let asset = make_nested_zip_asset(
            "D:/Assets",
            "pack.zip",
            "inner.zip/audio/test.wav",
            "test.wav",
            "wav",
        );

        assert_eq!(
            nested_zip_group_key(&asset),
            Some("D:/Assets/pack.zip::inner.zip".to_string())
        );
    }

    #[test]
    fn test_nested_zip_group_key_uses_deepest_nested_zip() {
        let asset = make_nested_zip_asset(
            "D:/Assets",
            "pack.zip",
            "level1.zip/level2.zip/audio/test.wav",
            "test.wav",
            "wav",
        );

        assert_eq!(
            nested_zip_group_key(&asset),
            Some("D:/Assets/pack.zip::level1.zip/level2.zip".to_string())
        );
    }

    #[test]
    fn test_load_asset_bytes_cached_supports_deep_nested_zips() {
        clear();
        let dir = tempfile::tempdir().unwrap();
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");
        let (zip_file, zip_entry, filename) = create_deep_nested_zip_with_wav(dir.path());
        let asset = make_nested_zip_asset(&folder_path, &zip_file, &zip_entry, &filename, "wav");

        let bytes = load_asset_bytes_cached(&asset).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], b"RIFF");
    }

    #[test]
    fn test_clear_does_not_panic() {
        clear();
    }
}
