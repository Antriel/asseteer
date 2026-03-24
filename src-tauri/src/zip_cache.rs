//! Multi-slot memory-aware cache for nested ZIP archives.
//!
//! Holds multiple decompressed nested ZIPs as shared immutable bytes,
//! bounded by a memory budget derived from available system RAM.
//! LRU eviction removes the least-recently-used unpinned entries when
//! the budget is exceeded. Per-entry reference counting prevents eviction
//! of entries that are actively being read.

use crate::models::Asset;
use crate::utils::{load_asset_bytes, resolve_zip_path};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
use zip::ZipArchive;

const MB: f64 = 1024.0 * 1024.0;
const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Default estimate for decompressed nested ZIP size before actual size is known.
/// INVARIANT: must be <= MIN_BUDGET_BYTES. On an empty cache the eviction check is
/// `0 + DEFAULT_ESTIMATED_SIZE > budget_bytes`; if this were true we'd try to evict
/// but find nothing to evict, then allow the load anyway — however the load would be
/// counted as over-budget from the start. Keeping this <= MIN_BUDGET_BYTES guarantees
/// an empty cache never triggers that over-budget path.
const DEFAULT_ESTIMATED_SIZE: usize = 1024 * 1024 * 1024; // 1 GB

/// Maximum memory budget cap
const MAX_BUDGET_BYTES: usize = 8 * 1024 * 1024 * 1024; // 8 GB

/// Minimum memory budget floor (must hold at least one entry).
/// Must be >= DEFAULT_ESTIMATED_SIZE — see comment on that constant.
const MIN_BUDGET_BYTES: usize = 1024 * 1024 * 1024; // 1 GB

/// Warn threshold: log if cache wait takes longer than this
const CACHE_WAIT_WARN_MS: u128 = 5000;

/// Fixed overhead for opening a ZIP and reading the central directory (conservative for HDDs).
const CACHE_LOAD_SEEK_MS: u128 = 4000;

/// Minimum expected decompression throughput (conservative for slow CPUs/HDDs).
/// Real hardware shows 40-80 MB/s; this is intentionally well below that.
const CACHE_LOAD_MIN_THROUGHPUT_MB_S: f64 = 15.0;

/// Compute the slow-load warning threshold in ms for a given decompressed size.
/// Formula: SEEK_MS + size_mb / MIN_THROUGHPUT_MB_S * 1000
fn cache_load_warn_threshold_ms(size_bytes: usize) -> u128 {
    let size_mb = size_bytes as f64 / MB;
    let throughput_ms = (size_mb / CACHE_LOAD_MIN_THROUGHPUT_MB_S * 1000.0) as u128;
    CACHE_LOAD_SEEK_MS + throughput_ms
}

// --- Cache data structures ---

enum EntryState {
    /// A thread is actively decompressing this nested ZIP
    Loading,
    /// Decompressed bytes are available
    Ready {
        bytes: Arc<Vec<u8>>,
        size_bytes: usize,
    },
}

struct CacheEntry {
    state: EntryState,
    /// Number of threads currently using this entry. Cannot evict if > 0.
    active_users: usize,
    /// LRU tracking: higher value = more recently used
    last_access: u64,
}

struct ZipCacheInner {
    entries: HashMap<String, CacheEntry>,
    access_counter: u64,
    total_cached_bytes: usize,
    /// Estimated bytes reserved for entries currently being decompressed (Loading state).
    /// Used to prevent concurrent loads from exceeding the budget.
    in_flight_bytes: usize,
    budget_bytes: usize,
}

static CACHE: Lazy<Mutex<ZipCacheInner>> = Lazy::new(|| {
    let budget = compute_budget_bytes();
    Mutex::new(ZipCacheInner {
        entries: HashMap::new(),
        access_counter: 0,
        total_cached_bytes: 0,
        in_flight_bytes: 0,
        budget_bytes: budget,
    })
});

/// Woken whenever an entry transitions (Loading → Ready, entry evicted/removed)
static CACHE_CHANGED: Condvar = Condvar::new();

/// RAII guard that decrements an entry's active_users on drop.
struct ActiveEntryGuard {
    key: String,
}

impl Drop for ActiveEntryGuard {
    fn drop(&mut self) {
        if let Ok(mut cache) = CACHE.lock() {
            if let Some(entry) = cache.entries.get_mut(&self.key) {
                entry.active_users = entry.active_users.saturating_sub(1);
            }
            CACHE_CHANGED.notify_all();
        }
    }
}

// --- Public API ---

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

    let (cached_bytes, _guard) =
        get_or_load_cached_bytes(&key, &outer_zip_path, nested_zip_path, DEFAULT_ESTIMATED_SIZE)?;
    load_entry_from_shared_zip_bytes(&cached_bytes, inner_entry, nested_zip_path)
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

/// Evict all cache entries that have no active users.
/// Safe to call while other categories are still processing — pinned entries survive.
pub fn evict_unpinned() {
    if let Ok(mut cache) = CACHE.lock() {
        let keys_to_evict: Vec<String> = cache
            .entries
            .iter()
            .filter(|(_, e)| e.active_users == 0 && matches!(e.state, EntryState::Ready { .. }))
            .map(|(k, _)| k.clone())
            .collect();

        for key in &keys_to_evict {
            if let Some(entry) = cache.entries.remove(key) {
                if let EntryState::Ready { size_bytes, .. } = entry.state {
                    cache.total_cached_bytes -= size_bytes;
                    println!(
                        "[ZipCache] EVICT key='{}' size_mb={:.1} reason=unpinned_cleanup",
                        key,
                        size_bytes as f64 / MB
                    );
                }
            }
        }

        // Remove stale Loading entries with no active users
        cache.entries.retain(|_, e| {
            !(matches!(e.state, EntryState::Loading) && e.active_users == 0)
        });

        CACHE_CHANGED.notify_all();
    }
}

/// Opaque guard that keeps a cached entry pinned. Drop to release for eviction.
pub struct CacheEntryGuard {
    _inner: ActiveEntryGuard,
}

/// Load nested ZIP bytes through the memory-budgeted cache for scan enumeration.
///
/// Returns shared bytes and a guard that keeps the entry pinned until dropped.
/// Used by scan/discovery to bound memory usage across parallel decompression tasks.
/// `size_hint` is the expected decompressed size (e.g. from ZIP entry metadata);
/// used for accurate budget reservation of in-flight loads.
pub fn load_for_scan(
    outer_zip_path: &str,
    nested_zip_entry: &str,
    size_hint: u64,
) -> Result<(Arc<Vec<u8>>, CacheEntryGuard), String> {
    let key = compose_cache_key(outer_zip_path, nested_zip_entry);
    let (bytes, guard) =
        get_or_load_cached_bytes(&key, outer_zip_path, nested_zip_entry, size_hint as usize)?;
    Ok((bytes, CacheEntryGuard { _inner: guard }))
}

/// Hard clear: evict everything. Only call when ALL processing is stopped.
#[allow(dead_code)]
pub fn clear() {
    if let Ok(mut cache) = CACHE.lock() {
        cache.entries.clear();
        cache.total_cached_bytes = 0;
        CACHE_CHANGED.notify_all();
    }
}

/// Current memory budget in bytes.
pub fn budget_bytes() -> usize {
    CACHE.lock().map(|c| c.budget_bytes).unwrap_or(0)
}

/// Total bytes currently cached.
#[allow(dead_code)]
pub fn cached_bytes() -> usize {
    CACHE.lock().map(|c| c.total_cached_bytes).unwrap_or(0)
}

/// Number of entries currently in the cache.
#[allow(dead_code)]
pub fn entry_count() -> usize {
    CACHE.lock().map(|c| c.entries.len()).unwrap_or(0)
}

// --- Core cache logic ---

/// Get cached bytes for a key, or load them if not cached.
/// Returns the shared bytes and an RAII guard that keeps the entry pinned.
/// `estimated_size` is the expected decompressed size, used for budget reservation
/// of in-flight loads. Pass 0 to use DEFAULT_ESTIMATED_SIZE.
fn get_or_load_cached_bytes(
    key: &str,
    outer_zip_path: &str,
    nested_zip_path: &str,
    estimated_size: usize,
) -> Result<(Arc<Vec<u8>>, ActiveEntryGuard), String> {
    let estimated_size = if estimated_size == 0 {
        DEFAULT_ESTIMATED_SIZE
    } else {
        estimated_size
    };
    let wait_started = Instant::now();

    loop {
        let mut cache = CACHE
            .lock()
            .map_err(|e| format!("Cache lock poisoned: {}", e))?;

        // Two-phase check: first extract data (immutable), then mutate.
        // This avoids borrow-checker conflicts from simultaneous field access.
        let state = cache.entries.get(key).map(|e| match &e.state {
            EntryState::Ready { bytes, .. } => Ok(bytes.clone()),
            EntryState::Loading => Err(()),
        });

        match state {
            Some(Ok(bytes)) => {
                // Cache hit — bump access counter and active users
                cache.access_counter += 1;
                let access = cache.access_counter;
                let entry = cache.entries.get_mut(key).unwrap();
                entry.active_users += 1;
                entry.last_access = access;

                let waited = wait_started.elapsed().as_millis();
                if waited > CACHE_WAIT_WARN_MS {
                    eprintln!(
                        "[ZipCache] WARN slow HIT key='{}' waited_ms={} entries={} cached_mb={:.0}",
                        key,
                        waited,
                        cache.entries.len(),
                        cache.total_cached_bytes as f64 / MB
                    );
                }

                return Ok((bytes, ActiveEntryGuard {
                    key: key.to_string(),
                }));
            }
            Some(Err(())) => {
                // Another thread is loading this key — wait for state change
                drop(
                    CACHE_CHANGED
                        .wait(cache)
                        .map_err(|e| format!("Cache lock poisoned: {}", e))?,
                );
                continue;
            }
            None => {
                // Cache miss — evict if needed, then check budget including in-flight loads
                evict_for_budget(&mut cache, estimated_size);

                let effective_usage = cache.total_cached_bytes + cache.in_flight_bytes;
                if effective_usage + estimated_size > cache.budget_bytes {
                    // Budget full (including in-flight loads) — wait for space
                    drop(
                        CACHE_CHANGED
                            .wait(cache)
                            .map_err(|e| format!("Cache lock poisoned: {}", e))?,
                    );
                    continue;
                }

                // Reserve budget for this load
                cache.in_flight_bytes += estimated_size;
                cache.access_counter += 1;
                let access = cache.access_counter;
                cache.entries.insert(
                    key.to_string(),
                    CacheEntry {
                        state: EntryState::Loading,
                        active_users: 1,
                        last_access: access,
                    },
                );
                drop(cache);
                break;
            }
        }
    }

    // Decompress outside the lock (expensive I/O)
    let load_started = Instant::now();
    let load_result = load_nested_zip_bytes_uncached(outer_zip_path, nested_zip_path);

    let mut cache = CACHE
        .lock()
        .map_err(|e| format!("Cache lock poisoned: {}", e))?;

    match load_result {
        Ok(bytes) => {
            let size_bytes = bytes.len();
            let shared = Arc::new(bytes);
            let load_ms = load_started.elapsed().as_millis();

            // Transition Loading → Ready: release reservation, add actual size
            if let Some(entry) = cache.entries.get_mut(key) {
                entry.state = EntryState::Ready {
                    bytes: shared.clone(),
                    size_bytes,
                };
            }
            cache.in_flight_bytes = cache.in_flight_bytes.saturating_sub(estimated_size);
            cache.total_cached_bytes += size_bytes;

            // Evict if over budget (actual size may differ from estimate)
            evict_for_budget(&mut cache, 0);

            if load_ms > cache_load_warn_threshold_ms(size_bytes) {
                eprintln!(
                    "[ZipCache] WARN slow LOAD key='{}' size_mb={:.1} load_ms={} entries={} cached_mb={:.0} in_flight_mb={:.0}",
                    key,
                    size_bytes as f64 / MB,
                    load_ms,
                    cache.entries.len(),
                    cache.total_cached_bytes as f64 / MB,
                    cache.in_flight_bytes as f64 / MB
                );
            } else {
                println!(
                    "[ZipCache] LOAD key='{}' size_mb={:.1} load_ms={} entries={} cached_mb={:.0}/{:.0} in_flight_mb={:.0}",
                    key,
                    size_bytes as f64 / MB,
                    load_ms,
                    cache.entries.len(),
                    cache.total_cached_bytes as f64 / MB,
                    cache.budget_bytes as f64 / MB,
                    cache.in_flight_bytes as f64 / MB
                );
            }

            CACHE_CHANGED.notify_all();
            Ok((shared, ActiveEntryGuard {
                key: key.to_string(),
            }))
        }
        Err(err) => {
            // Remove the Loading placeholder, release reservation
            cache.entries.remove(key);
            cache.in_flight_bytes = cache.in_flight_bytes.saturating_sub(estimated_size);
            CACHE_CHANGED.notify_all();
            Err(err)
        }
    }
}

/// Evict LRU unpinned entries until total usage (cached + in-flight + additional) fits within budget.
/// Called while holding the cache lock.
fn evict_for_budget(cache: &mut ZipCacheInner, additional: usize) {
    while cache.total_cached_bytes + cache.in_flight_bytes + additional > cache.budget_bytes {
        // Find the oldest Ready entry with no active users
        let victim = cache
            .entries
            .iter()
            .filter(|(_, e)| matches!(e.state, EntryState::Ready { .. }) && e.active_users == 0)
            .min_by_key(|(_, e)| e.last_access)
            .map(|(k, _)| k.clone());

        match victim {
            Some(victim_key) => {
                if let Some(entry) = cache.entries.remove(&victim_key) {
                    if let EntryState::Ready { size_bytes, .. } = entry.state {
                        cache.total_cached_bytes -= size_bytes;
                        println!(
                            "[ZipCache] EVICT key='{}' size_mb={:.1} reason=budget",
                            victim_key,
                            size_bytes as f64 / MB
                        );
                    }
                }
            }
            None => break, // all entries pinned — allow temporary over-budget
        }
    }
}

// --- Memory budget ---

fn compute_budget_bytes() -> usize {
    let available = get_available_memory_bytes();
    let budget = available / 2;
    let clamped = budget.clamp(MIN_BUDGET_BYTES, MAX_BUDGET_BYTES);
    println!(
        "[ZipCache] Memory budget: {:.1} GB (available: {:.1} GB)",
        clamped as f64 / GB,
        available as f64 / GB,
    );
    clamped
}

#[cfg(windows)]
fn get_available_memory_bytes() -> usize {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut mem: MEMORYSTATUSEX = std::mem::zeroed();
        mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut mem) != 0 {
            mem.ullAvailPhys as usize
        } else {
            4 * 1024 * 1024 * 1024 // fallback: assume 4 GB
        }
    }
}

#[cfg(not(windows))]
fn get_available_memory_bytes() -> usize {
    4 * 1024 * 1024 * 1024 // default: 4 GB
}

// --- Nested ZIP path parsing (unchanged) ---

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
    if let Some((nested_zip_name, remaining_path)) =
        split_first_nested_zip_segment(nested_zip_path)
    {
        let nested_bytes = read_entry_bytes(&mut archive, nested_zip_name, context)?;
        let nested_context = format!("{}/{}", context, nested_zip_name);
        let nested_archive = ZipArchive::new(Cursor::new(nested_bytes))
            .map_err(|e| format!("Failed to open nested zip {}: {}", nested_context, e))?;
        load_nested_zip_bytes_from_archive(nested_archive, remaining_path, &nested_context)
    } else {
        read_entry_bytes(&mut archive, nested_zip_path, context).map_err(|e| {
            format!(
                "Failed to read nested zip {} from {}: {}",
                nested_zip_path, context, e
            )
        })
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
            zip_compression: None,
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
            outer_zip_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            format!("{}/{}/{}", level1_name, level2_name, final_entry),
            "test.wav".to_string(),
        )
    }

    /// Create a simple outer ZIP containing a named inner ZIP with a WAV file inside.
    fn create_single_nested_zip(
        dir: &std::path::Path,
        outer_name: &str,
        inner_name: &str,
    ) -> (String, String, String) {
        let wav_path = create_test_wav(dir, &format!("wav_{}", outer_name));
        let wav_bytes = std::fs::read(wav_path).unwrap();
        let entry_name = "audio/test.wav";

        // Create inner ZIP containing the WAV
        let mut inner_bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut inner_bytes);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(entry_name, options).unwrap();
            zip.write_all(&wav_bytes).unwrap();
            zip.finish().unwrap();
        }

        // Create outer ZIP containing the inner ZIP
        let outer_path = dir.join(outer_name);
        {
            let file = std::fs::File::create(&outer_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file(inner_name, options).unwrap();
            zip.write_all(&inner_bytes).unwrap();
            zip.finish().unwrap();
        }

        (
            outer_name.to_string(),
            format!("{}/{}", inner_name, entry_name),
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
            zip_compression: None,
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
            zip_compression: None,
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

    #[test]
    fn test_multi_entry_cache() {
        // Don't clear — tests run in parallel and share global state.
        // Instead verify both entries load correctly and cache hits return identical data.
        let dir = tempfile::tempdir().unwrap();
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");

        let (zip_a, entry_a, fname_a) =
            create_single_nested_zip(dir.path(), "outer_a.zip", "inner_a.zip");
        let (zip_b, entry_b, fname_b) =
            create_single_nested_zip(dir.path(), "outer_b.zip", "inner_b.zip");

        let asset_a = make_nested_zip_asset(&folder_path, &zip_a, &entry_a, &fname_a, "wav");
        let asset_b = make_nested_zip_asset(&folder_path, &zip_b, &entry_b, &fname_b, "wav");

        // Load both — both should be cached simultaneously
        let bytes_a = load_asset_bytes_cached(&asset_a).unwrap();
        let bytes_b = load_asset_bytes_cached(&asset_b).unwrap();

        assert!(!bytes_a.is_empty());
        assert!(!bytes_b.is_empty());
        assert_eq!(&bytes_a[0..4], b"RIFF");
        assert_eq!(&bytes_b[0..4], b"RIFF");

        // Reload both — cache hits should return identical data
        let bytes_a2 = load_asset_bytes_cached(&asset_a).unwrap();
        let bytes_b2 = load_asset_bytes_cached(&asset_b).unwrap();
        assert_eq!(bytes_a, bytes_a2);
        assert_eq!(bytes_b, bytes_b2);
    }

    #[test]
    fn test_lru_eviction_on_budget() {
        // Test eviction logic directly on a local struct to avoid
        // interference from parallel tests sharing the global cache.
        let mut cache = ZipCacheInner {
            entries: HashMap::new(),
            access_counter: 0,
            total_cached_bytes: 0,
            in_flight_bytes: 0,
            budget_bytes: 300,
        };

        // Insert entry A (200 bytes, older)
        cache.access_counter += 1;
        cache.entries.insert(
            "key_a".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 200]),
                    size_bytes: 200,
                },
                active_users: 0,
                last_access: cache.access_counter,
            },
        );
        cache.total_cached_bytes = 200;

        // Insert entry B (200 bytes, newer)
        cache.access_counter += 1;
        cache.entries.insert(
            "key_b".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 200]),
                    size_bytes: 200,
                },
                active_users: 0,
                last_access: cache.access_counter,
            },
        );
        cache.total_cached_bytes = 400;

        // Over budget (400 > 300) — evict LRU
        evict_for_budget(&mut cache, 0);

        // A (older) should be evicted, B should remain
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key("key_b"));
        assert_eq!(cache.total_cached_bytes, 200);
    }

    #[test]
    fn test_pinned_entry_survives_eviction() {
        let mut cache = ZipCacheInner {
            entries: HashMap::new(),
            access_counter: 0,
            total_cached_bytes: 0,
            in_flight_bytes: 0,
            budget_bytes: 300,
        };

        // Entry A: pinned (active_users=1), older
        cache.access_counter += 1;
        cache.entries.insert(
            "pinned".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 200]),
                    size_bytes: 200,
                },
                active_users: 1, // pinned!
                last_access: cache.access_counter,
            },
        );

        // Entry B: unpinned, newer
        cache.access_counter += 1;
        cache.entries.insert(
            "unpinned".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 200]),
                    size_bytes: 200,
                },
                active_users: 0,
                last_access: cache.access_counter,
            },
        );
        cache.total_cached_bytes = 400;

        // Need to evict for 100 more bytes — only unpinned can be evicted
        evict_for_budget(&mut cache, 100);

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key("pinned"));
        assert_eq!(cache.total_cached_bytes, 200);
    }

    #[test]
    fn test_evict_unpinned_on_local_cache() {
        let mut cache = ZipCacheInner {
            entries: HashMap::new(),
            access_counter: 0,
            total_cached_bytes: 0,
            in_flight_bytes: 0,
            budget_bytes: 10000,
        };

        // Insert two unpinned entries
        cache.entries.insert(
            "idle_a".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 100]),
                    size_bytes: 100,
                },
                active_users: 0,
                last_access: 1,
            },
        );
        cache.entries.insert(
            "idle_b".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 100]),
                    size_bytes: 100,
                },
                active_users: 0,
                last_access: 2,
            },
        );
        // Insert one pinned entry
        cache.entries.insert(
            "active".into(),
            CacheEntry {
                state: EntryState::Ready {
                    bytes: Arc::new(vec![0u8; 100]),
                    size_bytes: 100,
                },
                active_users: 1,
                last_access: 3,
            },
        );
        cache.total_cached_bytes = 300;

        // Simulate evict_unpinned: remove all Ready entries with active_users == 0
        let keys_to_evict: Vec<String> = cache
            .entries
            .iter()
            .filter(|(_, e)| e.active_users == 0 && matches!(e.state, EntryState::Ready { .. }))
            .map(|(k, _)| k.clone())
            .collect();
        for key in &keys_to_evict {
            if let Some(entry) = cache.entries.remove(key) {
                if let EntryState::Ready { size_bytes, .. } = entry.state {
                    cache.total_cached_bytes -= size_bytes;
                }
            }
        }

        // Only the pinned entry should remain
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key("active"));
        assert_eq!(cache.total_cached_bytes, 100);
    }

    #[test]
    fn test_budget_bytes_returns_reasonable_value() {
        // Force lazy init
        drop(CACHE.lock());
        let budget = budget_bytes();
        assert!(budget >= MIN_BUDGET_BYTES);
        assert!(budget <= MAX_BUDGET_BYTES);
    }

    #[test]
    fn test_cache_hit_returns_same_data() {
        let dir = tempfile::tempdir().unwrap();
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");

        let (zip_file, zip_entry, filename) =
            create_single_nested_zip(dir.path(), "hit_test.zip", "inner.zip");
        let asset = make_nested_zip_asset(&folder_path, &zip_file, &zip_entry, &filename, "wav");

        // Load twice — second should be a cache hit returning identical data
        let bytes1 = load_asset_bytes_cached(&asset).unwrap();
        let bytes2 = load_asset_bytes_cached(&asset).unwrap();

        assert_eq!(bytes1, bytes2);
        assert_eq!(&bytes1[0..4], b"RIFF");
    }
}
