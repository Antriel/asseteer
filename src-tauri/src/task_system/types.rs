use crate::models::{ProcessingTask, TaskStatus, TaskType};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Control signals for task execution
#[derive(Debug, Clone)]
pub enum ControlSignal {
    Pause,
    Resume,
    Cancel,
}

/// Handle for controlling a running task
pub struct TaskHandle {
    pub task_id: i64,
    pub task_type: TaskType,
    pub status: Arc<RwLock<TaskStatus>>,
    pub control_tx: mpsc::Sender<ControlSignal>,
}

impl TaskHandle {
    pub fn new(
        task_id: i64,
        task_type: TaskType,
        control_tx: mpsc::Sender<ControlSignal>,
    ) -> Self {
        Self {
            task_id,
            task_type,
            status: Arc::new(RwLock::new(TaskStatus::Queued)),
            control_tx,
        }
    }

    pub async fn pause(&self) -> Result<(), String> {
        self.control_tx
            .send(ControlSignal::Pause)
            .await
            .map_err(|e| format!("Failed to send pause signal: {}", e))
    }

    pub async fn resume(&self) -> Result<(), String> {
        self.control_tx
            .send(ControlSignal::Resume)
            .await
            .map_err(|e| format!("Failed to send resume signal: {}", e))
    }

    pub async fn cancel(&self) -> Result<(), String> {
        self.control_tx
            .send(ControlSignal::Cancel)
            .await
            .map_err(|e| format!("Failed to send cancel signal: {}", e))
    }

    pub async fn get_status(&self) -> TaskStatus {
        self.status.read().await.clone()
    }

    pub async fn set_status(&self, status: TaskStatus) {
        *self.status.write().await = status;
    }
}

/// Context passed to task handlers
pub struct TaskContext {
    pub task: ProcessingTask,
    pub db: SqlitePool,
    pub control_rx: mpsc::Receiver<ControlSignal>,
    pub status_handle: Arc<RwLock<TaskStatus>>,
}

impl TaskContext {
    /// Check if task should pause or cancel
    pub async fn check_signals(&mut self) -> Result<bool, String> {
        // Try to receive control signal (non-blocking)
        match self.control_rx.try_recv() {
            Ok(ControlSignal::Cancel) => {
                *self.status_handle.write().await = TaskStatus::Cancelled;
                return Err("Task cancelled".to_string());
            }
            Ok(ControlSignal::Pause) => {
                *self.status_handle.write().await = TaskStatus::Paused;
                // Wait for resume or cancel
                loop {
                    match self.control_rx.recv().await {
                        Some(ControlSignal::Resume) => {
                            *self.status_handle.write().await = TaskStatus::Processing;
                            return Ok(true);
                        }
                        Some(ControlSignal::Cancel) => {
                            *self.status_handle.write().await = TaskStatus::Cancelled;
                            return Err("Task cancelled while paused".to_string());
                        }
                        Some(ControlSignal::Pause) => {
                            // Already paused, ignore
                        }
                        None => {
                            return Err("Control channel closed".to_string());
                        }
                    }
                }
            }
            Ok(ControlSignal::Resume) => {
                // Not paused, ignore
                Ok(false)
            }
            Err(mpsc::error::TryRecvError::Empty) => Ok(false),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err("Control channel closed".to_string())
            }
        }
    }

    /// Update task progress in database
    pub async fn update_progress(&self, current: i32, total: i32) -> Result<(), String> {
        sqlx::query(
            "UPDATE processing_tasks
             SET progress_current = ?, progress_total = ?
             WHERE id = ?",
        )
        .bind(current)
        .bind(total)
        .bind(self.task.id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to update progress: {}", e))?;
        Ok(())
    }
}

/// Result of task execution
pub struct TaskResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub output_data: Option<String>,
}

impl TaskResult {
    pub fn success(output_data: Option<String>) -> Self {
        Self {
            success: true,
            error_message: None,
            output_data,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            error_message: Some(message),
            output_data: None,
        }
    }
}
