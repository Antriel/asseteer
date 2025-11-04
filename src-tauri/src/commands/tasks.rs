use crate::models::{ProcessingTask, TaskType};
use crate::task_system::TaskManager;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Application state wrapper for TaskManager
pub type TaskManagerState = Arc<RwLock<TaskManager>>;

/// Start processing tasks
#[tauri::command]
pub async fn start_processing(
    task_manager: State<'_, TaskManagerState>,
    task_type: Option<String>,
    asset_type: Option<String>,
) -> Result<Vec<i64>, String> {
    let manager = task_manager.read().await;

    // Parse task type if provided
    let parsed_task_type = task_type.and_then(|tt| TaskType::from_str(&tt));

    manager.start_processing(parsed_task_type, asset_type).await
}

/// Pause a specific task
#[tauri::command]
pub async fn pause_task(
    task_manager: State<'_, TaskManagerState>,
    task_id: i64,
) -> Result<(), String> {
    let manager = task_manager.read().await;
    manager.pause_task(task_id).await
}

/// Resume a specific task
#[tauri::command]
pub async fn resume_task(
    task_manager: State<'_, TaskManagerState>,
    task_id: i64,
) -> Result<(), String> {
    let manager = task_manager.read().await;
    manager.resume_task(task_id).await
}

/// Cancel a specific task
#[tauri::command]
pub async fn cancel_task(
    task_manager: State<'_, TaskManagerState>,
    task_id: i64,
) -> Result<(), String> {
    let manager = task_manager.read().await;
    manager.cancel_task(task_id).await
}

/// Pause all active tasks
#[tauri::command]
pub async fn pause_all_tasks(task_manager: State<'_, TaskManagerState>) -> Result<(), String> {
    let manager = task_manager.read().await;
    manager.pause_all().await
}

/// Resume all paused tasks
#[tauri::command]
pub async fn resume_all_tasks(task_manager: State<'_, TaskManagerState>) -> Result<(), String> {
    let manager = task_manager.read().await;
    manager.resume_all().await
}

/// Get all tasks with optional status filter
#[tauri::command]
pub async fn get_tasks(
    task_manager: State<'_, TaskManagerState>,
    status: Option<String>,
) -> Result<Vec<ProcessingTask>, String> {
    let manager = task_manager.read().await;
    manager.get_tasks(status).await
}

/// Get a specific task by ID
#[tauri::command]
pub async fn get_task(
    task_manager: State<'_, TaskManagerState>,
    task_id: i64,
) -> Result<ProcessingTask, String> {
    let manager = task_manager.read().await;
    let tasks = manager.get_tasks(None).await?;
    tasks
        .into_iter()
        .find(|t| t.id == task_id)
        .ok_or_else(|| format!("Task {} not found", task_id))
}

/// Get task statistics
#[tauri::command]
pub async fn get_task_stats(
    task_manager: State<'_, TaskManagerState>,
) -> Result<TaskStats, String> {
    let manager = task_manager.read().await;
    let all_tasks = manager.get_tasks(None).await?;

    let stats = TaskStats {
        total: all_tasks.len(),
        pending: all_tasks.iter().filter(|t| t.status == "pending").count(),
        queued: all_tasks.iter().filter(|t| t.status == "queued").count(),
        processing: all_tasks.iter().filter(|t| t.status == "processing").count(),
        paused: all_tasks.iter().filter(|t| t.status == "paused").count(),
        complete: all_tasks.iter().filter(|t| t.status == "complete").count(),
        error: all_tasks.iter().filter(|t| t.status == "error").count(),
        cancelled: all_tasks.iter().filter(|t| t.status == "cancelled").count(),
    };

    Ok(stats)
}

#[derive(serde::Serialize)]
pub struct TaskStats {
    pub total: usize,
    pub pending: usize,
    pub queued: usize,
    pub processing: usize,
    pub paused: usize,
    pub complete: usize,
    pub error: usize,
    pub cancelled: usize,
}
