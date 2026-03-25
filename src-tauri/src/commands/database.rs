use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::database;

#[derive(Serialize)]
pub struct DbInfo {
    pub path: String,
    pub main_size: u64,
    pub wal_size: u64,
    pub page_count: i64,
    pub page_size: i64,
    pub freelist_count: i64,
    pub total_assets: i64,
    pub total_folders: i64,
}

#[tauri::command]
pub async fn get_db_info(state: State<'_, AppState>) -> Result<DbInfo, String> {
    let db_path = &state.db_path;

    // File sizes
    let main_size = std::fs::metadata(db_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let wal_path = format!("{}-wal", db_path);
    let wal_size = std::fs::metadata(&wal_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // SQLite PRAGMAs
    let page_count: (i64,) = sqlx::query_as("SELECT page_count FROM pragma_page_count")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let page_size: (i64,) = sqlx::query_as("SELECT page_size FROM pragma_page_size")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let freelist_count: (i64,) = sqlx::query_as("SELECT freelist_count FROM pragma_freelist_count")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    // Asset/folder counts
    let total_assets: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let total_folders: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM source_folders WHERE status = 'active'")
            .fetch_one(&state.pool)
            .await
            .map_err(|e| e.to_string())?;

    Ok(DbInfo {
        path: db_path.clone(),
        main_size,
        wal_size,
        page_count: page_count.0,
        page_size: page_size.0,
        freelist_count: freelist_count.0,
        total_assets: total_assets.0,
        total_folders: total_folders.0,
    })
}

#[tauri::command]
pub async fn vacuum_database(state: State<'_, AppState>) -> Result<(), String> {
    let pool = &state.pool;

    // Temporarily enable auto-checkpointing so VACUUM doesn't duplicate
    // the full DB size into the WAL file.
    sqlx::query("PRAGMA wal_autocheckpoint=1000")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let result = sqlx::query("VACUUM")
        .execute(pool)
        .await
        .map_err(|e| e.to_string());

    // Restore our normal auto-checkpoint setting regardless of VACUUM result
    let _ = sqlx::query("PRAGMA wal_autocheckpoint=50000")
        .execute(pool)
        .await;

    result?;

    // Checkpoint to clean up WAL after vacuum
    database::checkpoint_truncate(pool)
        .await
        .map_err(|e| e.to_string())?;

    println!("[DB] VACUUM completed successfully");
    Ok(())
}
