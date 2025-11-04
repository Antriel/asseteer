use crate::models::ProcessingResult;
use crate::AppState;
use image::{DynamicImage, GenericImageView, ImageReader};
use rayon::prelude::*;
use serde::Serialize;
use sqlx;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

#[derive(Clone, Serialize)]
pub struct ProcessProgress {
    pub total: usize,
    pub processed: usize,
    pub current_file: String,
    pub status: String,
}

/// Process a single image asset: extract dimensions and generate thumbnail
fn process_image_asset(path: &Path) -> Result<ProcessingResult, String> {
    // Load image
    let img = ImageReader::open(path)
        .map_err(|e| format!("Failed to open image: {}", e))?
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let (width, height) = img.dimensions();

    // Generate thumbnail (128px max dimension)
    let thumbnail = generate_thumbnail(&img, 128)?;

    Ok(ProcessingResult {
        width: Some(width as i32),
        height: Some(height as i32),
        duration_ms: None,
        sample_rate: None,
        channels: None,
        thumbnail_data: thumbnail,
        processing_status: "complete".to_string(),
        processing_error: None,
    })
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
    let src_image = FirImage::from_vec_u8(
        width,
        height,
        rgba.into_raw(),
        PixelType::U8x4,
    )
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

/// Process all pending image assets
#[tauri::command]
pub async fn process_pending_images(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<usize, String> {
    // Get all pending image assets
    let assets: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, path FROM assets
         WHERE processing_status = 'pending' AND asset_type = 'image'
         ORDER BY id"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let total = assets.len();
    if total == 0 {
        return Ok(0);
    }

    // Process in batches
    const BATCH_SIZE: usize = 50;
    let mut processed_count = 0;

    for (batch_idx, batch) in assets.chunks(BATCH_SIZE).enumerate() {
        // Process batch in parallel using rayon
        let results: Vec<_> = batch
            .par_iter()
            .map(|(id, path)| {
                let result = process_image_asset(Path::new(path));
                (*id, result)
            })
            .collect();

        // Update database
        let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

        for (id, result) in results {
            match result {
                Ok(pr) => {
                    sqlx::query(
                        "UPDATE assets
                         SET width = ?, height = ?, thumbnail_data = ?,
                             processing_status = ?, processing_error = NULL
                         WHERE id = ?"
                    )
                    .bind(pr.width)
                    .bind(pr.height)
                    .bind(pr.thumbnail_data)
                    .bind(pr.processing_status)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }
                Err(err) => {
                    sqlx::query(
                        "UPDATE assets
                         SET processing_status = 'error', processing_error = ?
                         WHERE id = ?"
                    )
                    .bind(err)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }
            }
        }

        tx.commit().await.map_err(|e| e.to_string())?;

        processed_count += batch.len();

        // Emit progress every batch
        let _ = app.emit(
            "process-progress",
            ProcessProgress {
                total,
                processed: processed_count,
                current_file: format!("Batch {}/{}", batch_idx + 1, (total + BATCH_SIZE - 1) / BATCH_SIZE),
                status: "processing".to_string(),
            },
        );
    }

    // Emit completion
    let _ = app.emit(
        "process-progress",
        ProcessProgress {
            total,
            processed: processed_count,
            current_file: String::new(),
            status: "complete".to_string(),
        },
    );

    Ok(processed_count)
}

/// Process a single audio asset: extract metadata
fn process_audio_asset(path: &Path) -> Result<ProcessingResult, String> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    // Open audio file
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open audio file: {}", e))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Set up hint from file extension
    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(ext.to_str().unwrap_or(""));
    }

    // Probe the media format
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Failed to probe audio format: {}", e))?;

    let format = probed.format;
    let track = format.default_track()
        .ok_or_else(|| "No audio track found".to_string())?;

    // Extract codec parameters
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.unwrap_or(0);
    let channels = codec_params.channels
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

    Ok(ProcessingResult {
        width: None,
        height: None,
        duration_ms: Some(duration_ms),
        sample_rate: Some(sample_rate as i32),
        channels: Some(channels),
        thumbnail_data: vec![],
        processing_status: "complete".to_string(),
        processing_error: None,
    })
}

/// Process all pending audio assets
#[tauri::command]
pub async fn process_pending_audio(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<usize, String> {
    // Get all pending audio assets
    let assets: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, path FROM assets
         WHERE processing_status = 'pending' AND asset_type = 'audio'
         ORDER BY id"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let total = assets.len();
    if total == 0 {
        return Ok(0);
    }

    // Process in batches
    const BATCH_SIZE: usize = 50;
    let mut processed_count = 0;

    for (batch_idx, batch) in assets.chunks(BATCH_SIZE).enumerate() {
        // Process batch in parallel using rayon
        let results: Vec<_> = batch
            .par_iter()
            .map(|(id, path)| {
                let result = process_audio_asset(Path::new(path));
                (*id, result)
            })
            .collect();

        // Update database
        let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

        for (id, result) in results {
            match result {
                Ok(pr) => {
                    sqlx::query(
                        "UPDATE assets
                         SET duration_ms = ?, sample_rate = ?, channels = ?,
                             processing_status = ?, processing_error = NULL
                         WHERE id = ?"
                    )
                    .bind(pr.duration_ms)
                    .bind(pr.sample_rate)
                    .bind(pr.channels)
                    .bind(pr.processing_status)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }
                Err(err) => {
                    sqlx::query(
                        "UPDATE assets
                         SET processing_status = 'error', processing_error = ?
                         WHERE id = ?"
                    )
                    .bind(err)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }
            }
        }

        tx.commit().await.map_err(|e| e.to_string())?;

        processed_count += batch.len();

        // Emit progress every batch
        let _ = app.emit(
            "process-progress",
            ProcessProgress {
                total,
                processed: processed_count,
                current_file: format!("Audio Batch {}/{}", batch_idx + 1, (total + BATCH_SIZE - 1) / BATCH_SIZE),
                status: "processing".to_string(),
            },
        );
    }

    // Emit completion
    let _ = app.emit(
        "process-progress",
        ProcessProgress {
            total,
            processed: processed_count,
            current_file: String::new(),
            status: "complete".to_string(),
        },
    );

    Ok(processed_count)
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
