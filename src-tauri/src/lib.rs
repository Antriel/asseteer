mod database;
mod models;
mod commands;
mod task_system;

use database::{initialize_db, close_db, DbPool};
use task_system::WorkQueue;
use tauri::Manager;
use std::sync::Arc;

/// Application state shared across all commands
pub struct AppState {
    pub pool: DbPool,
    pub work_queue: Arc<WorkQueue>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Get app data directory
            let app_dir = app.path().app_data_dir()
                .expect("Failed to get app data directory");

            // Create directory if it doesn't exist
            std::fs::create_dir_all(&app_dir)
                .expect("Failed to create app data directory");

            // Initialize database pool
            let db_path = app_dir.join("asseteer.db");
            let pool = tauri::async_runtime::block_on(async {
                initialize_db(db_path.to_str().unwrap()).await
            })
            .expect("Failed to initialize database");

            // Initialize WorkQueue
            let work_queue = Arc::new(WorkQueue::new());

            // Store pool and work queue in app state
            app.manage(AppState {
                pool: pool.clone(),
                work_queue: work_queue.clone(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan::start_scan,
            commands::search::search_assets,
            commands::search::get_asset_count,
            commands::process::start_processing_assets,
            commands::process::pause_processing,
            commands::process::resume_processing,
            commands::process::stop_processing,
            commands::process::get_processing_progress,
            commands::process::get_thumbnail,
            commands::process::get_pending_asset_count,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Exit => {
                    println!("[APP] Application exiting, cleaning up...");

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
