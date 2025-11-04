use crate::models::Asset;
use crate::task_system::ProcessingProgress;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

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

    // Get all pending assets (images without metadata OR audio without metadata)
    let images: Vec<Asset> = sqlx::query_as(
        "SELECT a.* FROM assets a
         LEFT JOIN image_metadata im ON a.id = im.asset_id
         WHERE a.asset_type = 'image' AND im.asset_id IS NULL
         ORDER BY a.id"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to query pending images: {}", e))?;

    let audio: Vec<Asset> = sqlx::query_as(
        "SELECT a.* FROM assets a
         LEFT JOIN audio_metadata am ON a.id = am.asset_id
         WHERE a.asset_type = 'audio' AND am.asset_id IS NULL
         ORDER BY a.id"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to query pending audio: {}", e))?;

    // Combine both lists
    let mut assets = images;
    assets.extend(audio);

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
pub async fn pause_processing(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    if !state.work_queue.is_running() {
        return Err("Processing is not running".to_string());
    }

    if state.work_queue.is_paused() {
        return Err("Processing is already paused".to_string());
    }

    state.work_queue.pause();
    println!("[ProcessingQueue] Processing paused");

    // Emit immediate progress update with paused state
    let progress = state.work_queue.get_progress();
    let _ = app.emit("processing-progress", progress);

    Ok(())
}

/// Resume processing
#[tauri::command]
pub async fn resume_processing(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    if !state.work_queue.is_running() {
        return Err("Processing is not running".to_string());
    }

    if !state.work_queue.is_paused() {
        return Err("Processing is not paused".to_string());
    }

    state.work_queue.resume();
    println!("[ProcessingQueue] Processing resumed");

    // Emit immediate progress update with resumed state
    let progress = state.work_queue.get_progress();
    let _ = app.emit("processing-progress", progress);

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

