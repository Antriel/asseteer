mod database;
mod models;
mod commands;

use database::{initialize_db, DbConnection};
use tauri::Manager;

/// Application state shared across all commands
pub struct AppState {
    pub db: DbConnection,
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

            // Initialize database
            let db_path = app_dir.join("asseteer.db");
            let db = initialize_db(db_path.to_str().unwrap())
                .expect("Failed to initialize database");

            // Store database in app state
            app.manage(AppState { db });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan::start_scan,
            commands::search::search_assets,
            commands::search::get_asset_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
