use crate::models::{Asset, ProcessingCategory};
use crate::task_system::ProcessingProgress;
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

/// Start processing pending assets for a specific category
#[tauri::command]
pub async fn start_processing(
    category: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // Parse category
    let cat = ProcessingCategory::from_str(&category)?;

    // Check if this category is already running
    if state.work_queue.is_running(cat).await {
        return Err(format!("Processing for category '{}' is already running", category));
    }

    // Get pending assets for this category
    let assets: Vec<Asset> = match cat {
        ProcessingCategory::Image => {
            sqlx::query_as(
                "SELECT a.* FROM assets a
                 LEFT JOIN image_metadata im ON a.id = im.asset_id
                 WHERE a.asset_type = 'image' AND im.asset_id IS NULL
                 ORDER BY a.id"
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| format!("Failed to query pending images: {}", e))?
        }
        ProcessingCategory::Audio => {
            sqlx::query_as(
                "SELECT a.* FROM assets a
                 LEFT JOIN audio_metadata am ON a.id = am.asset_id
                 WHERE a.asset_type = 'audio' AND am.asset_id IS NULL
                 ORDER BY a.id"
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| format!("Failed to query pending audio: {}", e))?
        }
    };

    if assets.is_empty() {
        return Err(format!("No pending assets to process for category '{}'", category));
    }

    println!("[ProcessingQueue] Starting processing of {} {} assets", assets.len(), category);

    // Start the work queue for this category
    state.work_queue
        .start(cat, assets, state.pool.clone(), app.clone())
        .await?;

    Ok(())
}

/// Pause processing for a specific category
#[tauri::command]
pub async fn pause_processing(
    category: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let cat = ProcessingCategory::from_str(&category)?;

    if !state.work_queue.is_running(cat).await {
        return Err(format!("Processing for category '{}' is not running", category));
    }

    if state.work_queue.is_paused(cat).await {
        return Err(format!("Processing for category '{}' is already paused", category));
    }

    state.work_queue.pause(cat).await;
    println!("[ProcessingQueue] Processing paused for category '{}'", category);

    // Emit immediate progress update with paused state
    let progress = state.work_queue.get_progress(Some(cat)).await;
    if let Some(prog) = progress.first() {
        let event_name = format!("processing-progress-{}", category);
        let _ = app.emit(&event_name, prog);
    }

    Ok(())
}

/// Resume processing for a specific category
#[tauri::command]
pub async fn resume_processing(
    category: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let cat = ProcessingCategory::from_str(&category)?;

    if !state.work_queue.is_running(cat).await {
        return Err(format!("Processing for category '{}' is not running", category));
    }

    if !state.work_queue.is_paused(cat).await {
        return Err(format!("Processing for category '{}' is not paused", category));
    }

    state.work_queue.resume(cat).await;
    println!("[ProcessingQueue] Processing resumed for category '{}'", category);

    // Emit immediate progress update with resumed state
    let progress = state.work_queue.get_progress(Some(cat)).await;
    if let Some(prog) = progress.first() {
        let event_name = format!("processing-progress-{}", category);
        let _ = app.emit(&event_name, prog);
    }

    Ok(())
}

/// Stop processing for a specific category
#[tauri::command]
pub async fn stop_processing(
    category: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cat = ProcessingCategory::from_str(&category)?;

    if !state.work_queue.is_running(cat).await {
        return Err(format!("Processing for category '{}' is not running", category));
    }

    state.work_queue.stop(cat).await;
    println!("[ProcessingQueue] Processing stopped for category '{}'", category);

    Ok(())
}

/// Get current processing progress for a category or all categories
#[tauri::command]
pub async fn get_processing_progress(
    category: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProcessingProgress>, String> {
    let cat = match category {
        Some(c) => Some(ProcessingCategory::from_str(&c)?),
        None => None,
    };

    Ok(state.work_queue.get_progress(cat).await)
}

