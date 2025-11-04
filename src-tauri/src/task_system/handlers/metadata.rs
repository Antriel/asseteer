use crate::models::{Asset, TaskProgress};
use crate::task_system::types::{TaskContext, TaskResult};
use image::{GenericImageView, ImageReader};
use std::path::Path;
use tauri::{AppHandle, Emitter};

/// Execute metadata extraction task
pub async fn execute(
    context: &mut TaskContext,
    asset: &Asset,
    app_handle: &AppHandle,
) -> Result<TaskResult, String> {
    // Check for pause/cancel signals
    context.check_signals().await?;

    let path = Path::new(&asset.path);
    let result = match asset.asset_type.as_str() {
        "image" => extract_image_metadata(path).await,
        "audio" => extract_audio_metadata(path).await,
        _ => Err(format!("Unsupported asset type: {}", asset.asset_type)),
    };

    match result {
        Ok((output, metadata_json)) => {
            // Update asset with metadata
            update_asset_metadata(&context.db, asset.id, &output).await?;

            // Update progress
            context.update_progress(1, 1).await?;

            // Emit progress event
            let _ = app_handle.emit(
                "task-progress",
                TaskProgress {
                    task_id: context.task.id,
                    asset_id: asset.id,
                    task_type: context.task.task_type.clone(),
                    status: "processing".to_string(),
                    progress_current: 1,
                    progress_total: 1,
                    current_file: asset.filename.clone(),
                },
            );

            Ok(TaskResult::success(Some(metadata_json)))
        }
        Err(e) => Ok(TaskResult::error(format!("Failed to extract metadata: {}", e))),
    }
}

enum MetadataOutput {
    Image { width: u32, height: u32 },
    Audio { duration_ms: i64, sample_rate: i32, channels: i32 },
}

/// Extract image metadata (dimensions)
async fn extract_image_metadata(path: &Path) -> Result<(MetadataOutput, String), String> {
    let path_buf = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let img = ImageReader::open(&path_buf)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let (width, height) = img.dimensions();

        let output = MetadataOutput::Image { width, height };
        let json = serde_json::json!({
            "width": width,
            "height": height,
        })
        .to_string();

        Ok((output, json))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Extract audio metadata (duration, sample rate, channels)
async fn extract_audio_metadata(path: &Path) -> Result<(MetadataOutput, String), String> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let path_buf = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        // Open audio file
        let file = std::fs::File::open(&path_buf)
            .map_err(|e| format!("Failed to open audio file: {}", e))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Set up hint from file extension
        let mut hint = Hint::new();
        if let Some(ext) = path_buf.extension() {
            hint.with_extension(ext.to_str().unwrap_or(""));
        }

        // Probe the media format
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| format!("Failed to probe audio format: {}", e))?;

        let format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| "No audio track found".to_string())?;

        // Extract codec parameters
        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(0) as i32;
        let channels = codec_params
            .channels
            .map(|c| c.count() as i32)
            .unwrap_or(0);

        // Calculate duration in milliseconds
        let duration_ms = if let Some(n_frames) = codec_params.n_frames {
            if sample_rate > 0 {
                (n_frames as f64 / sample_rate as f64 * 1000.0) as i64
            } else {
                0
            }
        } else {
            0
        };

        let output = MetadataOutput::Audio {
            duration_ms,
            sample_rate,
            channels,
        };

        let json = serde_json::json!({
            "duration_ms": duration_ms,
            "sample_rate": sample_rate,
            "channels": channels,
        })
        .to_string();

        Ok((output, json))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Update asset with extracted metadata
async fn update_asset_metadata(
    db: &sqlx::SqlitePool,
    asset_id: i64,
    output: &MetadataOutput,
) -> Result<(), String> {
    match output {
        MetadataOutput::Image { width, height } => {
            sqlx::query("UPDATE assets SET width = ?, height = ? WHERE id = ?")
                .bind(*width as i32)
                .bind(*height as i32)
                .bind(asset_id)
                .execute(db)
                .await
                .map_err(|e| format!("Failed to update image metadata: {}", e))?;
        }
        MetadataOutput::Audio {
            duration_ms,
            sample_rate,
            channels,
        } => {
            sqlx::query(
                "UPDATE assets SET duration_ms = ?, sample_rate = ?, channels = ? WHERE id = ?",
            )
            .bind(*duration_ms)
            .bind(*sample_rate)
            .bind(*channels)
            .bind(asset_id)
            .execute(db)
            .await
            .map_err(|e| format!("Failed to update audio metadata: {}", e))?;
        }
    }

    Ok(())
}
