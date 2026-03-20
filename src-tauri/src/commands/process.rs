use crate::models::{Asset, ProcessingCategory, ProcessingErrorDetail};
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

    // Get pending assets for this category (with folder_path from JOIN)
    let assets: Vec<Asset> = match cat {
        ProcessingCategory::Image => {
            sqlx::query_as(
                "SELECT a.*, sf.path as folder_path FROM assets a
                 JOIN source_folders sf ON a.folder_id = sf.id
                 LEFT JOIN image_metadata im ON a.id = im.asset_id
                 WHERE a.asset_type = 'image' AND im.asset_id IS NULL
                 ORDER BY a.folder_id, a.rel_path, a.zip_file, a.zip_entry"
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| format!("Failed to query pending images: {}", e))?
        }
        ProcessingCategory::Audio => {
            sqlx::query_as(
                "SELECT a.*, sf.path as folder_path FROM assets a
                 JOIN source_folders sf ON a.folder_id = sf.id
                 LEFT JOIN audio_metadata am ON a.id = am.asset_id
                 WHERE a.asset_type = 'audio' AND am.asset_id IS NULL
                 ORDER BY a.folder_id, a.rel_path, a.zip_file, a.zip_entry"
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| format!("Failed to query pending audio: {}", e))?
        }
        ProcessingCategory::Clap => {
            // CLAP embeddings - audio assets without embeddings
            // Sort by folder_id + rel_path + zip_file so files from the same ZIP/nested ZIP
            // are consecutive, enabling inner ZIP caching in the batch processor
            sqlx::query_as(
                "SELECT a.*, sf.path as folder_path FROM assets a
                 JOIN source_folders sf ON a.folder_id = sf.id
                 LEFT JOIN audio_embeddings ae ON a.id = ae.asset_id
                 WHERE a.asset_type = 'audio' AND ae.asset_id IS NULL
                 ORDER BY a.folder_id, a.rel_path, a.zip_file, a.zip_entry"
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| format!("Failed to query pending CLAP embeddings: {}", e))?
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
    app: AppHandle,
) -> Result<(), String> {
    let cat = ProcessingCategory::from_str(&category)?;

    if !state.work_queue.is_running(cat).await {
        return Err(format!("Processing for category '{}' is not running", category));
    }

    state.work_queue.stop(cat).await;
    println!("[ProcessingQueue] Processing stopped for category '{}'", category);

    // Emit immediate progress update with stopped state
    let progress = state.work_queue.get_progress(Some(cat)).await;
    if let Some(prog) = progress.first() {
        let event_name = format!("processing-progress-{}", category);
        let _ = app.emit(&event_name, prog);
    }

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

/// Get unresolved processing errors for a category
#[tauri::command]
pub async fn get_processing_errors(
    category: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProcessingErrorDetail>, String> {
    let errors = match &category {
        Some(cat) => {
            sqlx::query_as::<_, ProcessingErrorDetail>(
                "SELECT e.id, e.asset_id, a.filename, a.rel_path, sf.path as folder_path,
                        e.error_message, e.occurred_at, e.retry_count
                 FROM processing_errors e
                 JOIN assets a ON e.asset_id = a.id
                 JOIN source_folders sf ON a.folder_id = sf.id
                 WHERE e.resolved_at IS NULL AND e.category = ?
                 ORDER BY e.occurred_at DESC"
            )
            .bind(cat)
            .fetch_all(&state.pool)
            .await
        }
        None => {
            sqlx::query_as::<_, ProcessingErrorDetail>(
                "SELECT e.id, e.asset_id, a.filename, a.rel_path, sf.path as folder_path,
                        e.error_message, e.occurred_at, e.retry_count
                 FROM processing_errors e
                 JOIN assets a ON e.asset_id = a.id
                 JOIN source_folders sf ON a.folder_id = sf.id
                 WHERE e.resolved_at IS NULL
                 ORDER BY e.occurred_at DESC"
            )
            .fetch_all(&state.pool)
            .await
        }
    };

    errors.map_err(|e| format!("Failed to fetch errors: {}", e))
}

/// Retry failed assets for a category
#[tauri::command]
pub async fn retry_failed_assets(
    category: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<usize, String> {
    let cat = ProcessingCategory::from_str(&category)?;

    // Check if already running
    if state.work_queue.is_running(cat).await {
        return Err(format!(
            "Processing for category '{}' is already running",
            category
        ));
    }

    // Get assets with unresolved errors for this category (with folder_path)
    let assets: Vec<Asset> = sqlx::query_as(
        "SELECT DISTINCT a.*, sf.path as folder_path FROM assets a
         JOIN source_folders sf ON a.folder_id = sf.id
         JOIN processing_errors e ON a.id = e.asset_id
         WHERE e.category = ? AND e.resolved_at IS NULL
         ORDER BY a.folder_id, a.rel_path, a.zip_file, a.zip_entry"
    )
    .bind(&category)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("Failed to query failed assets: {}", e))?;

    if assets.is_empty() {
        return Err(format!(
            "No failed assets to retry for category '{}'",
            category
        ));
    }

    let count = assets.len();
    println!(
        "[ProcessingQueue] Retrying {} failed {} assets",
        count, category
    );

    // Increment retry count for these errors
    sqlx::query(
        "UPDATE processing_errors
         SET retry_count = retry_count + 1
         WHERE category = ? AND resolved_at IS NULL"
    )
    .bind(&category)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("Failed to update retry count: {}", e))?;

    // Start processing
    state
        .work_queue
        .start(cat, assets, state.pool.clone(), app)
        .await?;

    Ok(count)
}

/// Clear processing errors
#[tauri::command]
pub async fn clear_processing_errors(
    category: Option<String>,
    only_resolved: bool,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let result = match (&category, only_resolved) {
        (Some(cat), true) => {
            sqlx::query(
                "DELETE FROM processing_errors WHERE category = ? AND resolved_at IS NOT NULL"
            )
            .bind(cat)
            .execute(&state.pool)
            .await
        }
        (Some(cat), false) => {
            sqlx::query("DELETE FROM processing_errors WHERE category = ?")
                .bind(cat)
                .execute(&state.pool)
                .await
        }
        (None, true) => {
            sqlx::query("DELETE FROM processing_errors WHERE resolved_at IS NOT NULL")
                .execute(&state.pool)
                .await
        }
        (None, false) => {
            sqlx::query("DELETE FROM processing_errors")
                .execute(&state.pool)
                .await
        }
    };

    result
        .map(|r| r.rows_affected())
        .map_err(|e| format!("Failed to clear errors: {}", e))
}
