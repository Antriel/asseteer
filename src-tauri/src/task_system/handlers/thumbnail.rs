use crate::models::{Asset, TaskProgress};
use crate::task_system::types::{TaskContext, TaskResult};
use image::{DynamicImage, GenericImageView, ImageReader};
use std::path::Path;
use tauri::{AppHandle, Emitter};

/// Execute thumbnail generation task
pub async fn execute(
    context: &mut TaskContext,
    asset: &Asset,
    app_handle: &AppHandle,
) -> Result<TaskResult, String> {
    // Check if image type
    if asset.asset_type != "image" {
        return Ok(TaskResult::error(
            "Thumbnail task only supports image assets".to_string(),
        ));
    }

    // Check for pause/cancel signals
    context.check_signals().await?;

    // Load and process image
    let path = Path::new(&asset.path);
    match process_thumbnail(path).await {
        Ok((thumbnail_data, width, height)) => {
            // Encode thumbnail as base64 for JSON storage
            use base64::{Engine as _, engine::general_purpose};
            let thumbnail_b64 = general_purpose::STANDARD.encode(&thumbnail_data);
            let output = serde_json::json!({
                "thumbnail": thumbnail_b64,
                "width": width,
                "height": height,
            });

            // Update asset directly with thumbnail
            sqlx::query(
                "UPDATE assets
                 SET thumbnail_data = ?, width = ?, height = ?
                 WHERE id = ?",
            )
            .bind(thumbnail_data)
            .bind(width as i32)
            .bind(height as i32)
            .bind(asset.id)
            .execute(&context.db)
            .await
            .map_err(|e| format!("Failed to save thumbnail: {}", e))?;

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

            Ok(TaskResult::success(Some(output.to_string())))
        }
        Err(e) => Ok(TaskResult::error(format!("Failed to generate thumbnail: {}", e))),
    }
}

/// Process image and generate thumbnail
async fn process_thumbnail(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    // Run in blocking task since image processing is CPU-intensive
    let path_buf = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        // Load image
        let img = ImageReader::open(&path_buf)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let (width, height) = img.dimensions();

        // Generate thumbnail (128px max dimension)
        let thumbnail = generate_thumbnail(&img, 128)?;

        Ok((thumbnail, width, height))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Generate a thumbnail using fast_image_resize
fn generate_thumbnail(img: &DynamicImage, max_size: u32) -> Result<Vec<u8>, String> {
    use fast_image_resize::{images::Image as FirImage, PixelType, Resizer};

    let (width, height) = img.dimensions();

    // Calculate new dimensions maintaining aspect ratio
    let scale = (max_size as f32 / width.max(height) as f32).min(1.0);
    let new_width = (width as f32 * scale) as u32;
    let new_height = (height as f32 * scale) as u32;

    // Skip resize if image is already small enough
    if scale >= 1.0 {
        return encode_jpeg(img, width, height);
    }

    // Convert to RGBA8
    let rgba = img.to_rgba8();

    // Create source image
    let src_image = FirImage::from_vec_u8(width, height, rgba.into_raw(), PixelType::U8x4)
        .map_err(|e| format!("Failed to create source image: {}", e))?;

    // Create destination image
    let mut dst_image = FirImage::new(new_width, new_height, PixelType::U8x4);

    // Resize using Lanczos3 filter
    let mut resizer = Resizer::new();
    resizer
        .resize(&src_image, &mut dst_image, None)
        .map_err(|e| format!("Failed to resize: {}", e))?;

    // Encode as JPEG
    let mut buffer = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 85);
    encoder
        .encode(
            dst_image.buffer(),
            new_width,
            new_height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    Ok(buffer)
}

/// Helper to encode image as JPEG without resize
fn encode_jpeg(img: &DynamicImage, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let rgba = img.to_rgba8();
    let mut buffer = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 85);
    encoder
        .encode(
            &rgba.into_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;
    Ok(buffer)
}
