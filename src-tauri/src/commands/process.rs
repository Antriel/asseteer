use crate::models::Asset;
use crate::task_system::ProcessingProgress;
use crate::AppState;
use tauri::{AppHandle, State};

/// Start processing all pending assets (both images and audio)
#[tauri::command]
pub async fn start_processing_assets(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // Check if already running
    if state.work_queue.is_running() {
        return Err("Processing is already running".to_string());
    }

    // Get all pending assets
    let assets: Vec<Asset> = sqlx::query_as(
        "SELECT * FROM assets
         WHERE (thumbnail_data IS NULL OR width IS NULL OR height IS NULL OR duration_ms IS NULL)
         ORDER BY id"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to query pending assets: {}", e))?;

    if assets.is_empty() {
        return Err("No pending assets to process".to_string());
    }

    println!("[ProcessingQueue] Starting processing of {} assets", assets.len());

    // Start the work queue
    state.work_queue
        .start(assets, state.pool.clone(), app.clone())
        .await?;

    Ok(())
}

/// Pause processing
#[tauri::command]
pub async fn pause_processing(state: State<'_, AppState>) -> Result<(), String> {
    if !state.work_queue.is_running() {
        return Err("Processing is not running".to_string());
    }

    if state.work_queue.is_paused() {
        return Err("Processing is already paused".to_string());
    }

    state.work_queue.pause();
    println!("[ProcessingQueue] Processing paused");

    Ok(())
}

/// Resume processing
#[tauri::command]
pub async fn resume_processing(state: State<'_, AppState>) -> Result<(), String> {
    if !state.work_queue.is_running() {
        return Err("Processing is not running".to_string());
    }

    if !state.work_queue.is_paused() {
        return Err("Processing is not paused".to_string());
    }

    state.work_queue.resume();
    println!("[ProcessingQueue] Processing resumed");

    Ok(())
}

/// Stop processing completely
#[tauri::command]
pub async fn stop_processing(state: State<'_, AppState>) -> Result<(), String> {
    if !state.work_queue.is_running() {
        return Err("Processing is not running".to_string());
    }

    state.work_queue.stop().await;
    println!("[ProcessingQueue] Processing stopped");

    Ok(())
}

/// Get current processing progress
#[tauri::command]
pub async fn get_processing_progress(state: State<'_, AppState>) -> Result<ProcessingProgress, String> {
    Ok(state.work_queue.get_progress())
}

/// Get thumbnail data for a specific asset
#[tauri::command]
pub async fn get_thumbnail(state: State<'_, AppState>, asset_id: i64) -> Result<Vec<u8>, String> {
    let result: (Vec<u8>,) = sqlx::query_as(
        "SELECT thumbnail_data FROM assets WHERE id = ?"
    )
    .bind(asset_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(result.0)
}
