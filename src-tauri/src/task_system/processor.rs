/// Unified asset processor - handles both thumbnail generation and metadata extraction
use crate::models::Asset;
use image::{DynamicImage, GenericImageView, ImageReader};
use sqlx::SqlitePool;
use std::path::Path;

/// Result of processing an asset
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProcessingResult {
    pub asset_id: i64,
    pub success: bool,
    pub error: Option<String>,
}

/// Process a single asset (thumbnail + metadata combined)
pub async fn process_asset(asset: &Asset, db: &SqlitePool) -> ProcessingResult {
    let path = Path::new(&asset.path);

    match asset.asset_type.as_str() {
        "image" => process_image(asset.id, path, db).await,
        "audio" => process_audio(asset.id, path, db).await,
        _ => ProcessingResult {
            asset_id: asset.id,
            success: false,
            error: Some(format!("Unsupported asset type: {}", asset.asset_type)),
        },
    }
}

/// Process an image asset (thumbnail + dimensions in one go)
async fn process_image(asset_id: i64, path: &Path, db: &SqlitePool) -> ProcessingResult {
    let path_buf = path.to_path_buf();

    // Run CPU-intensive work in blocking thread
    let result = tokio::task::spawn_blocking(move || {
        // Load image
        let img = ImageReader::open(&path_buf)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let (width, height) = img.dimensions();

        // Generate thumbnail (128px max dimension)
        let thumbnail_data = generate_thumbnail(&img, 128)?;

        Ok::<_, String>((thumbnail_data, width, height))
    })
    .await;

    match result {
        Ok(Ok((thumbnail_data, width, height))) => {
            // Insert into image_metadata table
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            match sqlx::query(
                "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(asset_id)
            .bind(width as i32)
            .bind(height as i32)
            .bind(thumbnail_data)
            .bind(now)
            .execute(db)
            .await
            {
                Ok(_) => ProcessingResult {
                    asset_id,
                    success: true,
                    error: None,
                },
                Err(e) => ProcessingResult {
                    asset_id,
                    success: false,
                    error: Some(format!("Failed to save to database: {}", e)),
                },
            }
        }
        Ok(Err(e)) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(e),
        },
        Err(e) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(format!("Task join error: {}", e)),
        },
    }
}

/// Process an audio asset (metadata extraction)
async fn process_audio(asset_id: i64, path: &Path, db: &SqlitePool) -> ProcessingResult {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let path_buf = path.to_path_buf();

    // Run CPU-intensive work in blocking thread
    let result = tokio::task::spawn_blocking(move || {
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

        Ok::<_, String>((duration_ms, sample_rate, channels))
    })
    .await;

    match result {
        Ok(Ok((duration_ms, sample_rate, channels))) => {
            // Insert into audio_metadata table
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            match sqlx::query(
                "INSERT INTO audio_metadata (asset_id, duration_ms, sample_rate, channels, processed_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(asset_id)
            .bind(duration_ms)
            .bind(sample_rate)
            .bind(channels)
            .bind(now)
            .execute(db)
            .await
            {
                Ok(_) => ProcessingResult {
                    asset_id,
                    success: true,
                    error: None,
                },
                Err(e) => ProcessingResult {
                    asset_id,
                    success: false,
                    error: Some(format!("Failed to save to database: {}", e)),
                },
            }
        }
        Ok(Err(e)) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(e),
        },
        Err(e) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(format!("Task join error: {}", e)),
        },
    }
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
