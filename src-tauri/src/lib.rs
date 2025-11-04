mod database;
mod models;
mod commands;
mod task_system;

use database::{initialize_db, close_db, DbPool};
use task_system::TaskManager;
use tauri::Manager;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Application state shared across all commands
pub struct AppState {
    pub pool: DbPool,
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

            // Store pool in app state
            app.manage(AppState { pool: pool.clone() });

            // Initialize TaskManager
            let task_manager = TaskManager::new(pool.clone(), app.handle().clone());
            task_manager.start_checkpoint_loop();
            app.manage(Arc::new(RwLock::new(task_manager)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan::start_scan,
            commands::search::search_assets,
            commands::search::get_asset_count,
            commands::process::process_pending_images,
            commands::process::process_pending_audio,
            commands::process::get_thumbnail,
            commands::tasks::start_processing,
            commands::tasks::pause_task,
            commands::tasks::resume_task,
            commands::tasks::cancel_task,
            commands::tasks::pause_all_tasks,
            commands::tasks::resume_all_tasks,
            commands::tasks::get_tasks,
            commands::tasks::get_task,
            commands::tasks::get_task_stats,
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
