mod clap;
mod commands;
mod database;
mod models;
mod task_system;
pub mod thumbnail_worker;
mod utils;
mod zip_cache;

#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod concurrent_tests;

use database::{initialize_db, close_db, DbPool};
use task_system::WorkQueue;
use thumbnail_worker::ThumbnailWorkerHandle;
use tauri::Manager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Application state shared across all commands
pub struct AppState {
    pub pool: DbPool,
    pub db_path: String,
    pub work_queue: Arc<WorkQueue>,
    pub thumbnail_worker: ThumbnailWorkerHandle,
    /// Cached rescan previews, keyed by folder_id
    pub(crate) rescan_previews: Mutex<HashMap<i64, commands::rescan::CachedRescanPreview>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .setup(|app| {
            // Get app data directory
            let app_dir = app.path().app_data_dir()
                .expect("Failed to get app data directory");

            // Create directory if it doesn't exist
            std::fs::create_dir_all(&app_dir)
                .expect("Failed to create app data directory");

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
            let thumbnail_worker = thumbnail_worker::start_worker(
                app.handle(),
                pool.clone(),
            );

            // Store pool and work queue in app state
            app.manage(AppState {
                pool: pool.clone(),
                db_path: db_path.to_str().unwrap().to_string(),
                work_queue: work_queue.clone(),
                thumbnail_worker,
                rescan_previews: Mutex::new(HashMap::new()),
            });

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
            commands::folders::get_zip_dir_trees,
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
