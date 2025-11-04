use crate::models::{ProcessingTask, TaskProgress, TaskStatus, TaskType};
use crate::task_system::runner::TaskRunner;
use crate::task_system::types::TaskHandle;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time;

const CHECKPOINT_INTERVAL_SECS: u64 = 30;
const MAX_CONCURRENT_TASKS: usize = 4;

pub struct TaskManager {
    db: SqlitePool,
    active_tasks: Arc<RwLock<HashMap<i64, TaskHandle>>>,
    runner: Arc<TaskRunner>,
    app_handle: AppHandle,
}

impl TaskManager {
    pub fn new(db: SqlitePool, app_handle: AppHandle) -> Self {
        let runner = Arc::new(TaskRunner::new(db.clone()));
        Self {
            db,
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            runner,
            app_handle,
        }
    }

    /// Start the background checkpoint loop
    pub fn start_checkpoint_loop(&self) {
        let db = self.db.clone();
        let active_tasks = self.active_tasks.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(CHECKPOINT_INTERVAL_SECS));
            loop {
                interval.tick().await;
                if let Err(e) = Self::checkpoint_tasks(&db, &active_tasks).await {
                    eprintln!("Checkpoint error: {}", e);
                }
            }
        });
    }

    /// Start processing tasks of a specific type
    pub async fn start_processing(
        &self,
        task_type: Option<TaskType>,
        asset_type: Option<String>,
    ) -> Result<Vec<i64>, String> {
        // Query pending tasks
        let query = String::from(
            "SELECT * FROM processing_tasks WHERE status = 'pending' ORDER BY priority DESC, created_at ASC"
        );

        let tasks: Vec<ProcessingTask> = if let Some(tt) = task_type {
            let query = format!(
                "{} AND task_type = ? LIMIT 100",
                query
            );
            sqlx::query_as::<_, ProcessingTask>(&query)
                .bind(tt.as_str())
                .fetch_all(&self.db)
                .await
                .map_err(|e| format!("Failed to query tasks: {}", e))?
        } else if let Some(at) = asset_type {
            let query = format!(
                "{} AND asset_id IN (SELECT id FROM assets WHERE asset_type = ?) LIMIT 100",
                query
            );
            sqlx::query_as::<_, ProcessingTask>(&query)
                .bind(at)
                .fetch_all(&self.db)
                .await
                .map_err(|e| format!("Failed to query tasks: {}", e))?
        } else {
            let query = format!("{} LIMIT 100", query);
            sqlx::query_as::<_, ProcessingTask>(&query)
                .fetch_all(&self.db)
                .await
                .map_err(|e| format!("Failed to query tasks: {}", e))?
        };

        let task_ids: Vec<i64> = tasks.iter().map(|t| t.id).collect();

        // Queue tasks for execution
        for task in tasks {
            self.queue_task(task).await?;
        }

        Ok(task_ids)
    }

    /// Queue a single task for execution
    async fn queue_task(&self, task: ProcessingTask) -> Result<(), String> {
        // Update status to queued
        sqlx::query("UPDATE processing_tasks SET status = 'queued' WHERE id = ?")
            .bind(task.id)
            .execute(&self.db)
            .await
            .map_err(|e| format!("Failed to update task status: {}", e))?;

        // Spawn task execution
        let runner = self.runner.clone();
        let active_tasks = self.active_tasks.clone();
        let app_handle = self.app_handle.clone();
        let db = self.db.clone();

        tokio::spawn(async move {
            // Wait for slot if too many concurrent tasks
            loop {
                let count = active_tasks.read().await.len();
                if count < MAX_CONCURRENT_TASKS {
                    break;
                }
                time::sleep(Duration::from_millis(500)).await;
            }

            // Create task handle
            let task_type = TaskType::from_str(&task.task_type)
                .unwrap_or(TaskType::Metadata);
            let (control_tx, control_rx) = tokio::sync::mpsc::channel(10);
            let handle = TaskHandle::new(task.id, task_type.clone(), control_tx);

            // Register task
            active_tasks.write().await.insert(task.id, handle);

            // Run task
            let result = runner
                .run_task(task.clone(), control_rx, app_handle.clone())
                .await;

            // Complete task
            let (final_status, error_msg) = match result {
                Ok(_) => (TaskStatus::Complete, None),
                Err(e) => {
                    let status = if e.contains("cancelled") {
                        TaskStatus::Cancelled
                    } else {
                        TaskStatus::Error
                    };
                    (status, Some(e))
                }
            };

            // Update database
            if let Err(e) =
                Self::complete_task(&db, task.id, &final_status, error_msg).await
            {
                eprintln!("Failed to complete task {}: {}", task.id, e);
            }

            // Emit completion event
            let _ = app_handle.emit(
                "task-completed",
                TaskProgress {
                    task_id: task.id,
                    asset_id: task.asset_id,
                    task_type: task.task_type.clone(),
                    status: final_status.as_str().to_string(),
                    progress_current: task.progress_total,
                    progress_total: task.progress_total,
                    current_file: String::new(),
                },
            );

            // Unregister task
            active_tasks.write().await.remove(&task.id);
        });

        Ok(())
    }

    /// Pause a specific task
    pub async fn pause_task(&self, task_id: i64) -> Result<(), String> {
        let tasks = self.active_tasks.read().await;
        if let Some(handle) = tasks.get(&task_id) {
            handle.pause().await?;

            // Update database
            sqlx::query("UPDATE processing_tasks SET status = 'paused' WHERE id = ?")
                .bind(task_id)
                .execute(&self.db)
                .await
                .map_err(|e| format!("Failed to update task status: {}", e))?;

            Ok(())
        } else {
            Err(format!("Task {} not found or not running", task_id))
        }
    }

    /// Resume a specific task
    pub async fn resume_task(&self, task_id: i64) -> Result<(), String> {
        let tasks = self.active_tasks.read().await;
        if let Some(handle) = tasks.get(&task_id) {
            handle.resume().await?;

            // Update database
            sqlx::query("UPDATE processing_tasks SET status = 'processing' WHERE id = ?")
                .bind(task_id)
                .execute(&self.db)
                .await
                .map_err(|e| format!("Failed to update task status: {}", e))?;

            Ok(())
        } else {
            Err(format!("Task {} not found or not running", task_id))
        }
    }

    /// Cancel a specific task
    pub async fn cancel_task(&self, task_id: i64) -> Result<(), String> {
        let tasks = self.active_tasks.read().await;
        if let Some(handle) = tasks.get(&task_id) {
            handle.cancel().await?;
            Ok(())
        } else {
            Err(format!("Task {} not found or not running", task_id))
        }
    }

    /// Pause all active tasks
    pub async fn pause_all(&self) -> Result<(), String> {
        let tasks = self.active_tasks.read().await;
        for handle in tasks.values() {
            let _ = handle.pause().await; // Continue even if one fails
        }
        Ok(())
    }

    /// Resume all paused tasks
    pub async fn resume_all(&self) -> Result<(), String> {
        let tasks = self.active_tasks.read().await;
        for handle in tasks.values() {
            let _ = handle.resume().await; // Continue even if one fails
        }
        Ok(())
    }

    /// Get all tasks with optional status filter
    pub async fn get_tasks(&self, status: Option<String>) -> Result<Vec<ProcessingTask>, String> {
        let tasks = if let Some(s) = status {
            sqlx::query_as::<_, ProcessingTask>(
                "SELECT * FROM processing_tasks WHERE status = ? ORDER BY created_at DESC",
            )
            .bind(s)
            .fetch_all(&self.db)
            .await
        } else {
            sqlx::query_as::<_, ProcessingTask>(
                "SELECT * FROM processing_tasks ORDER BY created_at DESC",
            )
            .fetch_all(&self.db)
            .await
        };

        tasks.map_err(|e| format!("Failed to query tasks: {}", e))
    }

    /// Checkpoint active tasks to database
    async fn checkpoint_tasks(
        db: &SqlitePool,
        active_tasks: &Arc<RwLock<HashMap<i64, TaskHandle>>>,
    ) -> Result<(), String> {
        let tasks = active_tasks.read().await;
        for (task_id, handle) in tasks.iter() {
            let status = handle.get_status().await;
            sqlx::query("UPDATE processing_tasks SET status = ? WHERE id = ?")
                .bind(status.as_str())
                .bind(task_id)
                .execute(db)
                .await
                .map_err(|e| format!("Failed to checkpoint task {}: {}", task_id, e))?;
        }
        Ok(())
    }

    /// Mark task as complete in database
    async fn complete_task(
        db: &SqlitePool,
        task_id: i64,
        status: &TaskStatus,
        error: Option<String>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "UPDATE processing_tasks
             SET status = ?, completed_at = ?, error_message = ?
             WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(now)
        .bind(error)
        .bind(task_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to complete task: {}", e))?;

        Ok(())
    }
}
