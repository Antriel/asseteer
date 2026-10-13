//! Handing assets to other programs: native drag-out of a row into Audacity, a DAW,
//! Explorer, etc.
//!
//! The drop target needs a real, local file. Most assets already are one; the rest are
//! materialized into the drag cache *only when a drag actually starts* (never on select
//! or hover — that would write to disk for nothing nearly every time):
//! - ZIP entries (incl. nested ZIPs) have no path of their own, so they are extracted.
//! - Files on network drives are copied: the `drag` crate panics natively on UNC and
//!   mapped-drive paths (drag-rs#72), which would take the whole app down.
//!
//! Public contract (Scry's import reads it): every cached file has a sidecar
//! `<filename>.origin.json` beside it, `{ "path": "<origin>" }`, where the origin is exactly
//! what "Copy path" gives for that asset (`utils::asset_origin_path`). It exists whenever the
//! cached file does. Nothing is ever written next to a file that is used in place.

use crate::models::Asset;
use crate::zip_cache;
use crate::AppState;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::State;

/// Cache entries untouched for this long are removed on startup. Reusing an entry
/// refreshes it, so a sample dragged out regularly stays.
const DRAG_CACHE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Resolve a local file for `asset`, extracting/copying into `cache_dir` when needed.
pub fn ensure_local_file(asset: &Asset, cache_dir: &Path) -> Result<PathBuf, String> {
    if asset.zip_entry.is_none() {
        let path = PathBuf::from(crate::utils::resolve_asset_fs_path(asset));
        if !path.is_file() {
            return Err(format!("File not found: {}", path.display()));
        }
        if !is_network_path(&path) {
            return Ok(path);
        }
    }

    // Keyed by id + source mtime so a rescan that changed the file doesn't serve stale bytes;
    // the original filename is kept so the receiving program names the track sensibly.
    let dir = cache_dir.join(format!(
        "{}_{}",
        asset.id,
        asset.fs_modified_at.unwrap_or(0)
    ));
    let target = dir.join(&asset.filename);
    let sidecar = dir.join(format!("{}.origin.json", asset.filename));
    let origin = origin_json(asset);
    if target.is_file() {
        touch(&target);
        // Backfills entries cached before sidecars existed, and follows a renamed folder
        if std::fs::read(&sidecar).ok().as_deref() != Some(origin.as_slice()) {
            write_sidecar(&sidecar, &origin)?;
        }
        return Ok(target);
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create drag cache: {}", e))?;
    // Before the file itself, so a receiver never sees the file without it
    write_sidecar(&sidecar, &origin)?;
    // Written under a temporary name and renamed, so a half-written file is never reused.
    let partial = dir.join(format!("{}.partial", asset.filename));
    if asset.zip_entry.is_some() {
        let bytes = zip_cache::load_asset_bytes_cached(asset)?;
        std::fs::write(&partial, bytes)
    } else {
        std::fs::copy(crate::utils::resolve_asset_fs_path(asset), &partial).map(|_| ())
    }
    .map_err(|e| format!("Failed to write {}: {}", partial.display(), e))?;
    std::fs::rename(&partial, &target)
        .map_err(|e| format!("Failed to finalize {}: {}", target.display(), e))?;
    Ok(target)
}

/// Contents of the `.origin.json` sidecar naming where `asset` really lives.
fn origin_json(asset: &Asset) -> Vec<u8> {
    let json = serde_json::json!({ "path": crate::utils::asset_origin_path(asset) });
    serde_json::to_vec(&json).expect("a JSON object serializes")
}

/// Write a sidecar via a temporary name, so it is never seen half-written.
fn write_sidecar(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    std::fs::write(&partial, bytes)
        .and_then(|_| std::fs::rename(&partial, path))
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

/// Remove drag-cache entries not used within `DRAG_CACHE_MAX_AGE`.
pub fn prune_drag_cache(cache_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let dir = entry.path();
        let newest = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|f| f.metadata().and_then(|m| m.modified()).ok())
            .max();
        let stale = match newest {
            Some(t) => now.duration_since(t).unwrap_or_default() > DRAG_CACHE_MAX_AGE,
            None => true, // empty or unreadable
        };
        if stale {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// Bump a cached file's mtime so pruning treats it as recently used.
fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::options().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

#[cfg(windows)]
fn is_network_path(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOTE;

    let s = path.to_string_lossy().replace('/', "\\");
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        // Extended-length: `\\?\UNC\server\share` is remote, `\\?\C:\` is a drive
        return rest.starts_with(r"UNC\") || drive_is_remote(rest);
    }
    if s.starts_with(r"\\") {
        return true;
    }
    return drive_is_remote(&s);

    fn drive_is_remote(s: &str) -> bool {
        let b = s.as_bytes();
        if b.len() < 2 || b[1] != b':' || !b[0].is_ascii_alphabetic() {
            return false;
        }
        let root: Vec<u16> = std::ffi::OsStr::new(&format!("{}:\\", b[0] as char))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
    }
}

#[cfg(not(windows))]
fn is_network_path(_path: &Path) -> bool {
    false
}

/// Whether the primary mouse button is physically held right now. A drag only makes
/// sense while it is: Windows' drag loop treats "button up" as an immediate drop.
#[cfg(windows)]
fn primary_button_held() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};
    unsafe {
        // GetAsyncKeyState reads physical buttons; left-handed setups swap them.
        let key = if GetSystemMetrics(SM_SWAPBUTTON) != 0 {
            VK_RBUTTON
        } else {
            VK_LBUTTON
        };
        (GetAsyncKeyState(key as i32) as u16 & 0x8000) != 0
    }
}

#[cfg(not(windows))]
fn primary_button_held() -> bool {
    true
}

/// Assets for `asset_ids`, in that order.
async fn load_assets(state: &AppState, asset_ids: &[i64]) -> Result<Vec<Asset>, String> {
    // Chunked: a Shift+click range can exceed SQLite's bound-parameter limit
    let mut assets = Vec::with_capacity(asset_ids.len());
    for chunk in asset_ids.chunks(500) {
        let sql = format!(
            "SELECT a.*, sf.path as folder_path
             FROM assets a
             JOIN source_folders sf ON a.folder_id = sf.id
             WHERE a.id IN ({})",
            vec!["?"; chunk.len()].join(",")
        );
        let mut query = sqlx::query_as::<_, Asset>(&sql);
        for id in chunk {
            query = query.bind(id);
        }
        assets.extend(
            query
                .fetch_all(&state.pool)
                .await
                .map_err(|e| format!("Failed to load assets: {}", e))?,
        );
    }
    // Keep the list order the frontend sent, not the DB's
    let order: std::collections::HashMap<i64, usize> = asset_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    assets.sort_by_key(|a| order.get(&a.id).copied());
    Ok(assets)
}

/// Local files for `asset_ids`, in that order — see `ensure_local_file`.
async fn local_files(state: &AppState, asset_ids: &[i64]) -> Result<Vec<PathBuf>, String> {
    let assets = load_assets(state, asset_ids).await?;
    let cache_dir = state.drag_cache_dir.clone();
    tokio::task::spawn_blocking(move || {
        assets
            .iter()
            .map(|asset| ensure_local_file(asset, &cache_dir))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The assets' paths for "Copy path", in the given order — see `utils::asset_origin_path`.
/// Built here rather than in the frontend so they always match the drag-cache sidecars.
#[tauri::command]
pub async fn get_asset_origin_paths(
    state: State<'_, AppState>,
    asset_ids: Vec<i64>,
) -> Result<Vec<String>, String> {
    let assets = load_assets(&state, &asset_ids).await?;
    Ok(assets.iter().map(crate::utils::asset_origin_path).collect())
}

/// Put assets' files on the OS clipboard (Ctrl+C), so Ctrl+V in Explorer or a DAW
/// pastes real files. Same materialization as a drag. Returns how many were copied.
#[tauri::command]
pub async fn copy_assets_to_clipboard(
    window: tauri::Window,
    state: State<'_, AppState>,
    asset_ids: Vec<i64>,
) -> Result<usize, String> {
    if asset_ids.is_empty() {
        return Ok(0);
    }
    let paths = local_files(&state, &asset_ids).await?;
    let count = paths.len();
    set_clipboard_files(&window, paths)?;
    Ok(count)
}

/// Clipboard as Explorer's Ctrl+C leaves it: `CF_HDROP` plus a "Preferred DropEffect" of
/// copy, so pasting copies rather than moves our cached files.
#[cfg(windows)]
fn set_clipboard_files(window: &tauri::Window, paths: Vec<PathBuf>) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Ole::{CF_HDROP, DROPEFFECT_COPY};
    use windows_sys::Win32::UI::Shell::DROPFILES;

    /// A movable global block filled with `bytes`, owned by the clipboard once set.
    unsafe fn global_block(bytes: &[u8]) -> Result<*mut core::ffi::c_void, String> {
        let block = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
        if block.is_null() {
            return Err("Out of memory for clipboard".into());
        }
        let ptr = GlobalLock(block) as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        GlobalUnlock(block);
        Ok(block)
    }

    // DROPFILES header, then each path as UTF-16 + NUL, then a final NUL
    let header = std::mem::size_of::<DROPFILES>();
    let mut hdrop = vec![0u8; header];
    hdrop[0..4].copy_from_slice(&(header as u32).to_le_bytes()); // pFiles
    let f_wide = std::mem::offset_of!(DROPFILES, fWide);
    hdrop[f_wide..f_wide + 4].copy_from_slice(&1i32.to_le_bytes());
    for path in &paths {
        // Source paths are stored with `/`; not every paste target accepts that
        let path = std::ffi::OsString::from(path.to_string_lossy().replace('/', "\\"));
        for unit in path.encode_wide().chain(std::iter::once(0)) {
            hdrop.extend_from_slice(&unit.to_le_bytes());
        }
    }
    hdrop.extend_from_slice(&0u16.to_le_bytes());

    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
    unsafe {
        // Another app may be holding the clipboard for a moment
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(hwnd) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return Err("The clipboard is busy".into());
        }
        EmptyClipboard();
        let result = (|| {
            let files = global_block(&hdrop)?;
            if SetClipboardData(CF_HDROP as u32, files).is_null() {
                GlobalFree(files);
                return Err("Failed to set clipboard files".to_string());
            }
            let name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
            let effect = global_block(&DROPEFFECT_COPY.to_le_bytes())?;
            if SetClipboardData(RegisterClipboardFormatW(name.as_ptr()), effect).is_null() {
                GlobalFree(effect);
            }
            Ok(())
        })();
        CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
fn set_clipboard_files(_window: &tauri::Window, _paths: Vec<PathBuf>) -> Result<(), String> {
    Err("Copying files is only supported on Windows".into())
}

/// Start a native OS drag of assets' files out of the window, in the given order.
///
/// Called by the frontend once the pointer has moved past a threshold with the button
/// held. Resolves when the drag ends, with `"dropped"`, `"cancelled"`, or `"released"`
/// (the button came up while the files were being prepared, so no drag was started).
/// `image` is an optional `data:image/png;base64,...` drag preview.
#[tauri::command]
pub async fn start_asset_drag(
    app: tauri::AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    asset_ids: Vec<i64>,
    image: Option<String>,
) -> Result<&'static str, String> {
    if asset_ids.is_empty() {
        return Err("Nothing to drag".into());
    }
    let paths = local_files(&state, &asset_ids).await?;

    let image = match image
        .as_deref()
        .and_then(|s| s.strip_prefix("data:image/png;base64,"))
    {
        Some(data) => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("Bad drag image: {}", e))?
        }
        None => Vec::new(), // no preview; the OS falls back to its default
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        if !primary_button_held() {
            let _ = tx.send(Ok("released"));
            return;
        }
        // On Linux the crate drags from the GTK window rather than a raw window handle.
        #[cfg(target_os = "linux")]
        let window = match window.gtk_window() {
            Ok(w) => w,
            Err(e) => {
                let _ = tx.send(Err(format!("Drag failed: {}", e)));
                return;
            }
        };
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        // The crate unwraps some shell calls; keep an unexpected failure from killing the app.
        let started = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drag::start_drag(
                &window,
                drag::DragItem::Files(paths),
                drag::Image::Raw(image),
                move |result, _cursor| {
                    let _ = result_tx.send(result);
                },
                drag::Options::default(),
            )
        }));
        let outcome = match started {
            Ok(Ok(())) => match result_rx.try_recv() {
                Ok(drag::DragResult::Dropped) => Ok("dropped"),
                _ => Ok("cancelled"),
            },
            Ok(Err(e)) => Err(format!("Drag failed: {}", e)),
            Err(_) => Err("Drag failed unexpectedly".to_string()),
        };
        let _ = tx.send(outcome);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{create_test_wav, create_test_zip_with_image};

    fn asset_at(folder: &Path, filename: &str) -> Asset {
        Asset {
            id: 7,
            filename: filename.into(),
            folder_id: 1,
            rel_path: String::new(),
            zip_file: None,
            zip_entry: None,
            zip_compression: None,
            asset_type: "audio".into(),
            format: "wav".into(),
            file_size: 0,
            fs_modified_at: Some(123),
            created_at: 0,
            modified_at: 0,
            folder_path: folder.to_string_lossy().into_owned(),
        }
    }

    #[test]
    fn local_file_is_used_in_place() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        create_test_wav(src.path(), "kick.wav");

        let path = ensure_local_file(&asset_at(src.path(), "kick.wav"), cache.path()).unwrap();

        assert!(path.starts_with(src.path()));
        assert_eq!(
            std::fs::read_dir(cache.path()).unwrap().count(),
            0,
            "nothing written"
        );
        assert_eq!(
            std::fs::read_dir(src.path()).unwrap().count(),
            1,
            "no sidecar beside the user's file"
        );
    }

    /// The `path` recorded in the sidecar beside a cached file.
    fn sidecar_path(cached: &Path) -> String {
        let mut sidecar = cached.as_os_str().to_owned();
        sidecar.push(".origin.json");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(sidecar).unwrap()).unwrap();
        json["path"].as_str().unwrap().to_owned()
    }

    fn native(path: &str) -> String {
        path.replace(['/', '\\'], std::path::MAIN_SEPARATOR_STR)
    }

    fn zip_asset(src: &Path) -> (Asset, PathBuf) {
        let (zip_path, entry) = create_test_zip_with_image(src);
        let mut asset = asset_at(src, "test.png");
        asset.zip_file = Some(zip_path.file_name().unwrap().to_string_lossy().into_owned());
        asset.zip_entry = Some(entry);
        (asset, zip_path)
    }

    #[test]
    fn zip_entry_gets_a_sidecar_with_its_copy_path() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (asset, zip_path) = zip_asset(src.path());

        let path = ensure_local_file(&asset, cache.path()).unwrap();

        let origin = sidecar_path(&path);
        assert_eq!(origin, crate::utils::asset_origin_path(&asset));
        assert_eq!(
            origin,
            native(&format!("{}/images/test.png", zip_path.display()))
        );
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "no leftover temp files: {:?}", names);
    }

    #[test]
    fn nested_zip_entry_sidecar_names_the_full_chain() {
        use std::io::Write;
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let mut inner = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        inner.start_file("Sounds/coin.wav", options).unwrap();
        inner.write_all(b"RIFFcoin").unwrap();
        let inner = inner.finish().unwrap().into_inner();
        let mut outer =
            zip::ZipWriter::new(std::fs::File::create(src.path().join("Retro.zip")).unwrap());
        outer.start_file("sub.zip", options).unwrap();
        outer.write_all(&inner).unwrap();
        outer.finish().unwrap();
        let mut asset = asset_at(src.path(), "coin.wav");
        asset.zip_file = Some("Retro.zip".into());
        asset.zip_entry = Some("sub.zip/Sounds/coin.wav".into());

        let path = ensure_local_file(&asset, cache.path()).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"RIFFcoin");
        assert_eq!(
            sidecar_path(&path),
            native(&format!(
                "{}/Retro.zip/sub.zip/Sounds/coin.wav",
                src.path().display()
            ))
        );
    }

    #[test]
    fn reuse_backfills_and_refreshes_the_sidecar() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (mut asset, _) = zip_asset(src.path());
        let path = ensure_local_file(&asset, cache.path()).unwrap();
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(".origin.json");

        // An entry cached before sidecars existed
        std::fs::remove_file(&sidecar).unwrap();
        assert_eq!(ensure_local_file(&asset, cache.path()).unwrap(), path);
        assert_eq!(sidecar_path(&path), crate::utils::asset_origin_path(&asset));

        // The source folder was renamed: same id and mtime, so the cache entry is reused
        asset.folder_path = "/elsewhere/Packs".into();
        assert_eq!(ensure_local_file(&asset, cache.path()).unwrap(), path);
        assert_eq!(
            sidecar_path(&path),
            native("/elsewhere/Packs/test_archive.zip/images/test.png")
        );
    }

    #[test]
    fn missing_file_is_an_error() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        assert!(ensure_local_file(&asset_at(src.path(), "gone.wav"), cache.path()).is_err());
    }

    #[test]
    fn zip_entry_is_extracted_once_with_its_filename() {
        let src = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (zip_path, entry) = create_test_zip_with_image(src.path());
        let mut asset = asset_at(src.path(), "test.png");
        asset.zip_file = Some(zip_path.file_name().unwrap().to_string_lossy().into_owned());
        asset.zip_entry = Some(entry);

        let path = ensure_local_file(&asset, cache.path()).unwrap();
        assert!(path.starts_with(cache.path()));
        assert_eq!(path.file_name().unwrap(), "test.png");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));

        // Second drag reuses the extracted file even if the archive is gone.
        std::fs::remove_file(&zip_path).unwrap();
        assert_eq!(ensure_local_file(&asset, cache.path()).unwrap(), path);
    }

    #[test]
    fn prune_removes_only_stale_entries() {
        let cache = tempfile::tempdir().unwrap();
        let fresh = cache.path().join("1_0");
        let stale = cache.path().join("2_0");
        for dir in [&fresh, &stale] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("a.wav"), b"x").unwrap();
            std::fs::write(dir.join("a.wav.origin.json"), b"{}").unwrap();
        }
        let old = SystemTime::now() - DRAG_CACHE_MAX_AGE - Duration::from_secs(60);
        for file in ["a.wav", "a.wav.origin.json"] {
            std::fs::File::options()
                .write(true)
                .open(stale.join(file))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }

        prune_drag_cache(cache.path());

        assert!(fresh.exists());
        assert!(!stale.exists());
    }

    #[cfg(windows)]
    #[test]
    fn unc_paths_are_network() {
        assert!(is_network_path(Path::new(r"\\nas\audio\kick.wav")));
        assert!(is_network_path(Path::new("//nas/audio/kick.wav")));
        assert!(is_network_path(Path::new(r"\\?\UNC\nas\audio\kick.wav")));
        assert!(!is_network_path(Path::new(r"C:\audio\kick.wav")));
        assert!(!is_network_path(Path::new(r"\\?\C:\audio\kick.wav")));
    }
}
