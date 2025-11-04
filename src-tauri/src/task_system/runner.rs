use crate::models::{Asset, ProcessingTask, TaskProgress, TaskStatus, TaskType};
use crate::task_system::handlers;
use crate::task_system::types::{ControlSignal, TaskContext};
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

pub struct TaskRunner {
    db: SqlitePool,
}

impl TaskRunner {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// Run a single task
    pub async fn run_task(
        &self,
        task: ProcessingTask,
        control_rx: mpsc::Receiver<ControlSignal>,
        app_handle: AppHandle,
    ) -> Result<(), String> {
        // Update status to processing
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "UPDATE processing_tasks
             SET status = 'processing', started_at = ?
             WHERE id = ?",
        )
        .bind(now)
        .bind(task.id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("Failed to update task status: {}", e))?;

        // Emit started event
        let _ = app_handle.emit(
            "task-started",
            TaskProgress {
                task_id: task.id,
                asset_id: task.asset_id,
                task_type: task.task_type.clone(),
                status: "processing".to_string(),
                progress_current: 0,
                progress_total: task.progress_total,
                current_file: String::new(),
            },
        );

        // Load asset
        let asset = self.load_asset(task.asset_id).await?;

        // Create task context
        let status_handle = Arc::new(RwLock::new(TaskStatus::Processing));
        let mut context = TaskContext {
            task: task.clone(),
            db: self.db.clone(),
            control_rx,
            status_handle: status_handle.clone(),
        };

        // Determine task type and execute
        let task_type = TaskType::from_str(&task.task_type)
            .ok_or_else(|| format!("Unknown task type: {}", task.task_type))?;

        let result = match task_type {
            TaskType::Thumbnail => {
                handlers::thumbnail::execute(&mut context, &asset, &app_handle).await
            }
            TaskType::Metadata => {
                handlers::metadata::execute(&mut context, &asset, &app_handle).await
            }
        };

        // Check for cancellation after execution
        let final_status = status_handle.read().await.clone();
        if final_status == TaskStatus::Cancelled {
            return Err("Task cancelled".to_string());
        }

        // Process result
        match result {
            Ok(task_result) => {
                if task_result.success {
                    // Update asset with output data if provided
                    if let Some(output) = task_result.output_data {
                        self.update_asset_with_output(
                            task.asset_id,
                            &task.task_type,
                            &output,
                        )
                        .await?;
                    }
                    Ok(())
                } else {
                    Err(task_result
                        .error_message
                        .unwrap_or_else(|| "Unknown error".to_string()))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Load asset from database
    async fn load_asset(&self, asset_id: i64) -> Result<Asset, String> {
        sqlx::query_as::<_, Asset>("SELECT * FROM assets WHERE id = ?")
            .bind(asset_id)
            .fetch_one(&self.db)
            .await
            .map_err(|e| format!("Failed to load asset: {}", e))
    }

    /// Update asset with task output
    async fn update_asset_with_output(
        &self,
        asset_id: i64,
        task_type: &str,
        output_json: &str,
    ) -> Result<(), String> {
        // Parse output based on task type
        match task_type {
            "thumbnail" => {
                // Output contains thumbnail data as base64
                let data: serde_json::Value = serde_json::from_str(output_json)
                    .map_err(|e| format!("Failed to parse output: {}", e))?;

                if let Some(thumbnail_b64) = data.get("thumbnail").and_then(|v| v.as_str()) {
                    use base64::{Engine as _, engine::general_purpose};
                    let thumbnail_data = general_purpose::STANDARD.decode(thumbnail_b64)
                        .map_err(|e| format!("Failed to decode thumbnail: {}", e))?;

                    sqlx::query("UPDATE assets SET thumbnail_data = ? WHERE id = ?")
                        .bind(thumbnail_data)
                        .bind(asset_id)
                        .execute(&self.db)
                        .await
                        .map_err(|e| format!("Failed to update thumbnail: {}", e))?;
                }
            }
            "metadata" => {
                // Output contains metadata fields
                let data: serde_json::Value = serde_json::from_str(output_json)
                    .map_err(|e| format!("Failed to parse output: {}", e))?;

                // Update various metadata fields
                if let Some(width) = data.get("width").and_then(|v| v.as_i64()) {
                    sqlx::query("UPDATE assets SET width = ? WHERE id = ?")
                        .bind(width as i32)
                        .bind(asset_id)
                        .execute(&self.db)
                        .await
                        .map_err(|e| format!("Failed to update width: {}", e))?;
                }

                if let Some(height) = data.get("height").and_then(|v| v.as_i64()) {
                    sqlx::query("UPDATE assets SET height = ? WHERE id = ?")
                        .bind(height as i32)
                        .bind(asset_id)
                        .execute(&self.db)
                        .await
                        .map_err(|e| format!("Failed to update height: {}", e))?;
                }

                if let Some(duration_ms) = data.get("duration_ms").and_then(|v| v.as_i64()) {
                    sqlx::query("UPDATE assets SET duration_ms = ? WHERE id = ?")
                        .bind(duration_ms)
                        .bind(asset_id)
                        .execute(&self.db)
                        .await
                        .map_err(|e| format!("Failed to update duration: {}", e))?;
                }

                if let Some(sample_rate) = data.get("sample_rate").and_then(|v| v.as_i64()) {
                    sqlx::query("UPDATE assets SET sample_rate = ? WHERE id = ?")
                        .bind(sample_rate as i32)
                        .bind(asset_id)
                        .execute(&self.db)
                        .await
                        .map_err(|e| format!("Failed to update sample rate: {}", e))?;
                }

                if let Some(channels) = data.get("channels").and_then(|v| v.as_i64()) {
                    sqlx::query("UPDATE assets SET channels = ? WHERE id = ?")
                        .bind(channels as i32)
                        .bind(asset_id)
                        .execute(&self.db)
                        .await
                        .map_err(|e| format!("Failed to update channels: {}", e))?;
                }
            }
            _ => {
                return Err(format!("Unknown task type for output update: {}", task_type));
            }
        }

        Ok(())
    }
}
