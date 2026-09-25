mod clap;
mod cli;
mod commands;
mod database;
mod models;
mod task_system;
pub mod thumbnail_worker;
mod utils;
mod zip_cache;

#[cfg(test)]
mod concurrent_tests;
#[cfg(test)]
mod test_helpers;

use database::{close_db, initialize_db, DbPool};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use task_system::WorkQueue;
use tauri::Manager;
use thumbnail_worker::ThumbnailWorkerHandle;

/// Application state shared across all commands
pub struct AppState {
    pub pool: DbPool,
    pub db_path: String,
    pub work_queue: Arc<WorkQueue>,
    pub thumbnail_worker: ThumbnailWorkerHandle,
    /// Cached rescan previews, keyed by folder_id
    pub(crate) rescan_previews: Mutex<HashMap<i64, commands::rescan::CachedRescanPreview>>,
}

/// Expose the WebView2 CDP endpoint in debug builds so Playwright can attach with
/// `chromium.connectOverCDP` and drive the real app — see `tests/CLAUDE.md`.
///
/// Set here rather than through `additionalBrowserArgs` in `tauri.conf.json`, which
/// would ship a debug port in release builds too. Windows/WebView2 only; the variable
/// is inert elsewhere. An existing value is respected so the harness can pick its port.
/// 9223, not the usual 9222, so it doesn't collide with Scry's harness.
#[cfg(debug_assertions)]
fn enable_cdp() {
    const VAR: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
    if std::env::var_os(VAR).is_none() {
        std::env::set_var(VAR, "--remote-debugging-port=9223");
    }
}

#[cfg(not(debug_assertions))]
fn enable_cdp() {}

/// Where to park the window for `--offscreen`: fully left of the leftmost monitor.
///
/// A fixed offset is not enough — with a monitor at negative x it can leave part of the
/// window on screen. Offscreen rather than hidden or minimized: CDP screenshots come
/// from the renderer, which can stop producing frames for a window Windows considers gone.
fn offscreen_position(
    window: &tauri::WebviewWindow,
    window_size: Option<(u32, u32)>,
) -> tauri::PhysicalPosition<i32> {
    const MARGIN: i32 = 400;

    let scale = window.scale_factor().unwrap_or(1.0);
    let width = match window_size {
        Some((w, _)) => (f64::from(w) * scale).round() as i32,
        None => window.outer_size().map(|s| s.width as i32).unwrap_or(1600),
    };

    let (left, top) = window
        .available_monitors()
        .ok()
        .filter(|monitors| !monitors.is_empty())
        .map(|monitors| {
            (
                monitors.iter().map(|m| m.position().x).min().unwrap_or(0),
                monitors.iter().map(|m| m.position().y).min().unwrap_or(0),
            )
        })
        .unwrap_or((0, 0));

    tauri::PhysicalPosition::new(left - width - MARGIN, top)
}

/// Build the `main` window. `tauri.conf.json` marks it `"create": false` because
/// `--profile-dir` (the webview data directory) can only be set before the window exists.
/// It is created hidden and shown once sized and parked, so it never flashes.
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    let cli = cli::args();
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .expect("tauri.conf.json must define a window labelled `main`")
        .clone();

    let mut builder = tauri::WebviewWindowBuilder::from_config(app, &config)?;
    if let Some((width, height)) = cli.window_size {
        builder = builder.inner_size(f64::from(width), f64::from(height));
    }
    if let Some(dir) = &cli.profile_dir {
        builder = builder.data_directory(std::path::PathBuf::from(dir));
    }

    let window = builder.build()?;
    if cli.offscreen {
        window.set_position(offscreen_position(&window, cli.window_size))?;
    }
    window.show()?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must happen before the webview is created.
    enable_cdp();
    let cli = cli::args();

    let mut builder = tauri::Builder::default();
    // The window-state plugin persists to the real config dir; a harness run would
    // otherwise save its offscreen position into the user's own app.
    if !cli.isolated() {
        builder = builder.plugin(tauri_plugin_window_state::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .setup(|app| {
            // App data directory, unless the harness isolated it with --data-dir
            let app_dir = match &cli.data_dir {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app
                    .path()
                    .app_data_dir()
                    .expect("Failed to get app data directory"),
            };

            // Create directory if it doesn't exist
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");

            // Initialize uv module with app data directory
            clap::uv::init_app_data_dir(app_dir.clone());

            // Store app handle for CLAP event emission
            clap::init_app_handle(app.handle().clone());

            // Initialize database pool
            let db_path = app_dir.join("asseteer.db");
            let pool = tauri::async_runtime::block_on(async {
                initialize_db(db_path.to_str().unwrap()).await
            })
            .expect("Failed to initialize database");

            // Initialize WorkQueue
            let work_queue = Arc::new(WorkQueue::new());

            // Start thumbnail background worker
            let thumbnail_worker = thumbnail_worker::start_worker(app.handle(), pool.clone());

            // Store pool and work queue in app state
            app.manage(AppState {
                pool: pool.clone(),
                db_path: db_path.to_str().unwrap().to_string(),
                work_queue: work_queue.clone(),
                thumbnail_worker,
                rescan_previews: Mutex::new(HashMap::new()),
            });

            create_main_window(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::assets::get_asset_bytes,
            commands::assets::request_thumbnails,
            commands::assets::cancel_thumbnails,
            commands::assets::clear_thumbnail_queue,
            commands::scan::add_folder,
            commands::folders::list_folders,
            commands::folders::remove_folder,
            commands::folders::rename_folder,
            commands::folders::update_search_excludes,
            commands::rescan::preview_rescan,
            commands::rescan::apply_rescan,
            commands::process::start_processing,
            commands::process::pause_processing,
            commands::process::resume_processing,
            commands::process::stop_processing,
            commands::process::get_processing_progress,
            commands::process::get_processing_errors,
            commands::process::retry_failed_assets,
            commands::process::clear_processing_errors,
            // CLAP semantic search commands
            commands::search::search_audio_semantic,
            commands::search::search_audio_by_similarity,
            commands::search::get_pending_clap_count,
            commands::search::check_clap_server,
            commands::search::start_clap_server,
            commands::search::get_clap_server_info,
            commands::search::get_clap_cache_size,
            commands::search::clear_clap_cache,
            commands::search::get_clap_log_dir,
            commands::search::check_clap_setup_state,
            commands::search::invalidate_embedding_cache,
            // Database management commands
            commands::database::get_db_info,
            commands::database::get_db_path,
            commands::database::vacuum_database,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Exit => {
                    println!("[APP] Application exiting, cleaning up...");

                    // Stop CLAP server if we started it
                    clap::stop_server();

                    // Close the database pool properly
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        let pool = state.pool.clone();
                        tauri::async_runtime::block_on(async {
                            close_db(pool).await;
                        });
                    } else {
                        println!("[APP] Could not get AppState");
                    }
                }
                _ => {}
            }
        });
}
