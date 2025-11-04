mod database;
mod models;
mod commands;

use database::{initialize_db, close_db, DbPool};
use tauri::Manager;

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
            app.manage(AppState { pool });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan::start_scan,
            commands::search::search_assets,
            commands::search::get_asset_count,
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
