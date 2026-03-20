/// Unified asset processor - handles both thumbnail generation and metadata extraction
use crate::clap::{embedding_to_blob, ensure_server_running, get_clap_client};
use crate::models::Asset;
use crate::utils::resolve_asset_fs_path;
use crate::zip_cache;
use image::{DynamicImage, GenericImageView};
use sqlx::SqlitePool;
use std::time::{Duration, Instant};

/// Timeout for processing a single asset (30 seconds)
const PROCESSING_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for nested ZIP assets (2 minutes — cache fills can take tens of seconds)
const NESTED_ZIP_PROCESSING_TIMEOUT: Duration = Duration::from_secs(120);

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Result of processing an asset
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessingResult {
    pub asset_id: i64,
    pub success: bool,
    pub error: Option<String>,
}

/// Process a single asset (thumbnail + metadata combined)
pub async fn process_asset(asset: &Asset, db: &SqlitePool) -> ProcessingResult {
    match asset.asset_type.as_str() {
        "image" => process_image(asset, db).await,
        "audio" => process_audio(asset, db).await,
        _ => ProcessingResult {
            asset_id: asset.id,
            success: false,
            error: Some(format!("Unsupported asset type: {}", asset.asset_type)),
        },
    }
}

/// Process an image asset (dimensions only, thumbnails are generated lazily)
async fn process_image(asset: &Asset, db: &SqlitePool) -> ProcessingResult {
    let asset_id = asset.id;

    let asset_clone = asset.clone();

    // Run CPU-intensive work in blocking thread with timeout
    let result = tokio::time::timeout(
        PROCESSING_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            // Load image bytes (cached for nested ZIPs)
            let bytes = zip_cache::load_asset_bytes_cached(&asset_clone)?;

            // Load image from memory
            let img = image::load_from_memory(&bytes)
                .map_err(|e| format!("Failed to decode image: {}", e))?;

            let (width, height) = img.dimensions();

            Ok::<_, String>((width, height))
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok((width, height)))) => {
            let now = unix_now();

            // Insert dimensions only; don't overwrite thumbnail if lazy loading already generated one
            match sqlx::query(
                "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
                 VALUES (?, ?, ?, NULL, ?)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     width = excluded.width,
                     height = excluded.height,
                     processed_at = excluded.processed_at",
            )
            .bind(asset_id)
            .bind(width as i32)
            .bind(height as i32)
            .bind(now)
            .execute(db)
            .await
            {
                Ok(_) => {
                    // Mark any existing errors as resolved
                    let _ = sqlx::query(
                        "UPDATE processing_errors SET resolved_at = ? WHERE asset_id = ? AND resolved_at IS NULL"
                    )
                    .bind(now)
                    .bind(asset_id)
                    .execute(db)
                    .await;

                    ProcessingResult {
                        asset_id,
                        success: true,
                        error: None,
                    }
                }
                Err(e) => ProcessingResult {
                    asset_id,
                    success: false,
                    error: Some(format!("Failed to save to database: {}", e)),
                },
            }
        }
        Ok(Ok(Err(e))) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(e),
        },
        Ok(Err(e)) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(format!("Task join error: {}", e)),
        },
        Err(_) => ProcessingResult {
            asset_id,
            success: false,
            error: Some("Processing timed out".to_string()),
        },
    }
}

/// Generate a thumbnail for an asset on demand.
/// Returns (width, height, thumbnail_data) on success.
pub async fn generate_thumbnail_for_asset(
    asset: &Asset,
) -> Result<(i32, i32, Vec<u8>), String> {
    let asset_clone = asset.clone();

    tokio::time::timeout(
        PROCESSING_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            let bytes = zip_cache::load_asset_bytes_cached(&asset_clone)?;
            let img = image::load_from_memory(&bytes)
                .map_err(|e| format!("Failed to decode image: {}", e))?;
            let (width, height) = img.dimensions();
            let thumbnail_data = generate_thumbnail(&img, 128)?;
            Ok((width as i32, height as i32, thumbnail_data))
        }),
    )
    .await
    .map_err(|_| "Thumbnail generation timed out".to_string())?
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Process an audio asset (metadata extraction)
async fn process_audio(asset: &Asset, db: &SqlitePool) -> ProcessingResult {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let asset_id = asset.id;
    let uses_nested_zip = zip_cache::is_nested_zip_asset(asset);
    let asset_clone = asset.clone();

    /// Warn threshold: log if audio asset load takes longer than this
    const AUDIO_LOAD_WARN_MS: u128 = 10000;
    /// Warn threshold: log if audio probe takes longer than this
    const AUDIO_PROBE_WARN_MS: u128 = 5000;

    let blocking_task = tokio::task::spawn_blocking(move || {
        let load_started = Instant::now();

        // Load audio bytes (cached for nested ZIPs)
        let bytes = zip_cache::load_asset_bytes_cached(&asset_clone)?;

        let load_ms = load_started.elapsed().as_millis();
        if load_ms > AUDIO_LOAD_WARN_MS {
            eprintln!(
                "[AudioProcess] WARN slow load asset_id={} file='{}' nested={} bytes_mb={:.2} load_ms={}",
                asset_clone.id,
                asset_clone.filename,
                zip_cache::is_nested_zip_asset(&asset_clone),
                bytes.len() as f64 / (1024.0 * 1024.0),
                load_ms
            );
        }

        // Create a cursor from the bytes for reading
        let cursor = std::io::Cursor::new(bytes);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        // Set up hint from file extension
        let mut hint = Hint::new();
        hint.with_extension(&asset_clone.format);

        let probe_started = Instant::now();

        // Probe the media format
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| format!("Failed to probe audio format: {}", e))?;

        let probe_ms = probe_started.elapsed().as_millis();
        if probe_ms > AUDIO_PROBE_WARN_MS {
            eprintln!(
                "[AudioProcess] WARN slow probe asset_id={} file='{}' probe_ms={}",
                asset_clone.id, asset_clone.filename, probe_ms
            );
        }

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
    });

    let timeout = if uses_nested_zip { NESTED_ZIP_PROCESSING_TIMEOUT } else { PROCESSING_TIMEOUT };
    let result: Result<(i64, i32, i32), String> = match tokio::time::timeout(timeout, blocking_task).await {
        Ok(join_result) => match join_result {
            Ok(inner) => inner,
            Err(e) => Err(format!("Task join error: {}", e)),
        },
        Err(_) => Err(format!("Processing timed out after {}s", timeout.as_secs())),
    };

    match result {
        Ok((duration_ms, sample_rate, channels)) => {
            // Insert into audio_metadata table
            let now = unix_now();

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
                Ok(_) => {
                    // Mark any existing errors as resolved
                    let _ = sqlx::query(
                        "UPDATE processing_errors SET resolved_at = ? WHERE asset_id = ? AND resolved_at IS NULL"
                    )
                    .bind(now)
                    .bind(asset_id)
                    .execute(db)
                    .await;

                    ProcessingResult {
                        asset_id,
                        success: true,
                        error: None,
                    }
                }
                Err(e) => ProcessingResult {
                    asset_id,
                    success: false,
                    error: Some(format!("Failed to save to database: {}", e)),
                },
            }
        }
        Err(e) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(e),
        },
    }
}

/// Process CLAP embedding for an audio asset (single - fallback)
#[allow(dead_code)]
pub async fn process_clap_embedding(asset: &Asset, db: &SqlitePool) -> ProcessingResult {
    let results = process_clap_embedding_batch(&[asset.clone()], db).await;
    results.into_iter().next().unwrap_or(ProcessingResult {
        asset_id: asset.id,
        success: false,
        error: Some("Batch returned no results".to_string()),
    })
}

/// Process CLAP embeddings for a batch of audio assets
pub async fn process_clap_embedding_batch(assets: &[Asset], db: &SqlitePool) -> Vec<ProcessingResult> {
    // Ensure CLAP server is running (this is a no-op if already running)
    if let Err(e) = ensure_server_running().await {
        return assets
            .iter()
            .map(|a| ProcessingResult {
                asset_id: a.id,
                success: false,
                error: Some(format!("CLAP server unavailable: {}", e)),
            })
            .collect();
    }

    let client = get_clap_client().await;

    // Partition assets into filesystem files and ZIP files
    let mut fs_indices = Vec::new();
    let mut fs_paths = Vec::new();
    let mut zip_indices = Vec::new();
    let mut zip_items: Vec<(Vec<u8>, String)> = Vec::new();
    let mut load_errors: Vec<(usize, String)> = Vec::new();

    for (i, asset) in assets.iter().enumerate() {
        if asset.zip_entry.is_some() {
            match zip_cache::load_asset_bytes_cached(asset) {
                Ok(bytes) => {
                    zip_indices.push(i);
                    zip_items.push((bytes, asset.filename.clone()));
                }
                Err(e) => {
                    load_errors.push((i, format!("Failed to load asset bytes: {}", e)));
                }
            }
        } else {
            fs_indices.push(i);
            // Resolve absolute filesystem path from folder_path + rel_path + filename
            fs_paths.push(resolve_asset_fs_path(asset));
        }
    }

    // Initialize results
    let mut results: Vec<Option<ProcessingResult>> = vec![None; assets.len()];

    // Fill in load errors
    for (idx, err) in load_errors {
        results[idx] = Some(ProcessingResult {
            asset_id: assets[idx].id,
            success: false,
            error: Some(err),
        });
    }

    // Batch request for filesystem files
    if !fs_paths.is_empty() {
        match client.embed_audio_batch_paths(&fs_paths).await {
            Ok(embeddings) => {
                for (batch_idx, embed_result) in embeddings.into_iter().enumerate() {
                    let asset_idx = fs_indices[batch_idx];
                    results[asset_idx] = Some(match embed_result {
                        Ok(embedding) => store_embedding(assets[asset_idx].id, &embedding, db).await,
                        Err(e) => ProcessingResult {
                            asset_id: assets[asset_idx].id,
                            success: false,
                            error: Some(e),
                        },
                    });
                }
            }
            Err(e) => {
                // Batch failed entirely - mark all as failed
                for &idx in &fs_indices {
                    results[idx] = Some(ProcessingResult {
                        asset_id: assets[idx].id,
                        success: false,
                        error: Some(format!("Batch request failed: {}", e)),
                    });
                }
            }
        }
    }

    // Batch request for ZIP files
    if !zip_items.is_empty() {
        match client.embed_audio_batch_bytes(zip_items).await {
            Ok(embeddings) => {
                for (batch_idx, embed_result) in embeddings.into_iter().enumerate() {
                    let asset_idx = zip_indices[batch_idx];
                    results[asset_idx] = Some(match embed_result {
                        Ok(embedding) => store_embedding(assets[asset_idx].id, &embedding, db).await,
                        Err(e) => ProcessingResult {
                            asset_id: assets[asset_idx].id,
                            success: false,
                            error: Some(e),
                        },
                    });
                }
            }
            Err(e) => {
                for &idx in &zip_indices {
                    results[idx] = Some(ProcessingResult {
                        asset_id: assets[idx].id,
                        success: false,
                        error: Some(format!("Batch upload failed: {}", e)),
                    });
                }
            }
        }
    }

    // Convert to final results
    results
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            r.unwrap_or(ProcessingResult {
                asset_id: assets[i].id,
                success: false,
                error: Some("No result produced".to_string()),
            })
        })
        .collect()
}

/// Store an embedding in the database and resolve errors
async fn store_embedding(asset_id: i64, embedding: &[f32], db: &SqlitePool) -> ProcessingResult {
    let blob = embedding_to_blob(embedding);
    let now = unix_now();

    match sqlx::query(
        "INSERT INTO audio_embeddings (asset_id, embedding, created_at)
         VALUES (?, ?, ?)
         ON CONFLICT (asset_id) DO UPDATE SET
             embedding = excluded.embedding,
             created_at = excluded.created_at",
    )
    .bind(asset_id)
    .bind(&blob)
    .bind(now)
    .execute(db)
    .await
    {
        Ok(_) => {
            let _ = sqlx::query(
                "UPDATE processing_errors SET resolved_at = ? WHERE asset_id = ? AND category = 'clap' AND resolved_at IS NULL",
            )
            .bind(now)
            .bind(asset_id)
            .execute(db)
            .await;

            ProcessingResult {
                asset_id,
                success: true,
                error: None,
            }
        }
        Err(e) => ProcessingResult {
            asset_id,
            success: false,
            error: Some(format!("Failed to save embedding: {}", e)),
        },
    }
}

/// Generate a thumbnail using fast_image_resize and encode as WebP
fn generate_thumbnail(img: &DynamicImage, max_size: u32) -> Result<Vec<u8>, String> {
    use fast_image_resize::{images::Image as FirImage, PixelType, Resizer};

    let (width, height) = img.dimensions();

    // Calculate new dimensions maintaining aspect ratio
    let scale = (max_size as f32 / width.max(height) as f32).min(1.0);
    let new_width = ((width as f32 * scale) as u32).max(1);
    let new_height = ((height as f32 * scale) as u32).max(1);

    // Skip resize if image is already small enough
    if scale >= 1.0 {
        return encode_webp(img, width, height);
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

    // Encode as WebP (supports alpha channel)
    encode_webp_from_rgba(dst_image.buffer(), new_width, new_height)
}

/// Encode image as WebP without resize (preserves alpha channel)
fn encode_webp(img: &DynamicImage, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let rgba = img.to_rgba8();
    encode_webp_from_rgba(&rgba.into_raw(), width, height)
}

/// Encode RGBA buffer as lossy WebP with quality 85
fn encode_webp_from_rgba(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    use webp::{Encoder, WebPMemory};

    // WebP encoder panics on invalid dimensions (VP8_ENC_ERROR_BAD_DIMENSION)
    if width == 0 || height == 0 {
        return Err(format!("Invalid dimensions for WebP encoding: {}x{}", width, height));
    }
    // libwebp max dimension is 16383
    if width > 16383 || height > 16383 {
        return Err(format!("Dimensions too large for WebP encoding: {}x{} (max 16383)", width, height));
    }

    // Create encoder from RGBA pixels
    let encoder: Encoder = Encoder::from_rgba(rgba, width, height);

    // Encode with quality 85 (0-100 scale, 85 is good balance)
    let encoded: WebPMemory = encoder.encode(85.0);

    // Return as Vec<u8>
    Ok(encoded.to_vec())
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    /// Helper: create an asset with folder_path populated for test use
    fn make_test_asset_with_path(filename: &str, folder_path: &str, folder_id: i64, asset_type: &str, format: &str) -> Asset {
        let mut asset = make_asset(filename, folder_id, "", asset_type, format);
        asset.folder_path = folder_path.to_string();
        asset
    }

    // -----------------------------------------------------------------------
    // process_asset – images
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_process_image_extracts_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");
        let folder_id = insert_source_folder(&db, &folder_path, "test").await;
        create_test_png(dir.path(), "test.png");

        let mut asset = make_test_asset_with_path("test.png", &folder_path, folder_id, "image", "png");
        asset.id = insert_asset(&db, &asset).await;

        let result = process_asset(&asset, &db).await;
        assert!(result.success, "Should succeed: {:?}", result.error);

        let (width, height): (i32, i32) =
            sqlx::query_as("SELECT width, height FROM image_metadata WHERE asset_id = ?")
                .bind(asset.id)
                .fetch_one(&db)
                .await
                .expect("Should have image_metadata row");

        assert_eq!(width, 64);
        assert_eq!(height, 48);
    }

    #[tokio::test]
    async fn test_process_image_stores_null_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");
        let folder_id = insert_source_folder(&db, &folder_path, "test").await;
        create_test_png(dir.path(), "test.png");

        let mut asset = make_test_asset_with_path("test.png", &folder_path, folder_id, "image", "png");
        asset.id = insert_asset(&db, &asset).await;

        process_asset(&asset, &db).await;

        let (thumb,): (Option<Vec<u8>>,) =
            sqlx::query_as("SELECT thumbnail_data FROM image_metadata WHERE asset_id = ?")
                .bind(asset.id)
                .fetch_one(&db)
                .await
                .unwrap();

        assert!(
            thumb.is_none(),
            "Thumbnail should be NULL after processing (lazy loading)"
        );
    }

    #[tokio::test]
    async fn test_process_image_does_not_overwrite_existing_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");
        let folder_id = insert_source_folder(&db, &folder_path, "test").await;
        create_test_png(dir.path(), "test.png");

        let mut asset = make_test_asset_with_path("test.png", &folder_path, folder_id, "image", "png");
        asset.id = insert_asset(&db, &asset).await;

        // Pre-populate with a fake thumbnail
        let fake_thumb = vec![1, 2, 3, 4];
        sqlx::query(
            "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
             VALUES (?, 10, 10, ?, 0)",
        )
        .bind(asset.id)
        .bind(&fake_thumb)
        .execute(&db)
        .await
        .unwrap();

        // Process again – ON CONFLICT should update dimensions but keep thumbnail
        process_asset(&asset, &db).await;

        let row: (i32, i32, Option<Vec<u8>>) = sqlx::query_as(
            "SELECT width, height, thumbnail_data FROM image_metadata WHERE asset_id = ?",
        )
        .bind(asset.id)
        .fetch_one(&db)
        .await
        .unwrap();

        // Dimensions updated to real values
        assert_eq!(row.0, 64);
        assert_eq!(row.1, 48);
        // process_image's ON CONFLICT doesn't touch thumbnail_data, so it stays
        assert!(row.2.is_some(), "Existing thumbnail should be preserved");
    }

    // -----------------------------------------------------------------------
    // generate_thumbnail_for_asset
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_generate_thumbnail_produces_webp() {
        let dir = tempfile::tempdir().unwrap();
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");
        create_test_png(dir.path(), "test.png");
        let asset = Asset {
            id: 1,
            filename: "test.png".into(),
            folder_id: 1,
            rel_path: "".into(),
            zip_file: None,
            zip_entry: None,
            asset_type: "image".into(),
            format: "png".into(),
            file_size: 0,
            fs_modified_at: None,
            created_at: 0,
            modified_at: 0,
            folder_path,
        };

        let (w, h, data) = generate_thumbnail_for_asset(&asset).await.unwrap();
        assert_eq!(w, 64);
        assert_eq!(h, 48);
        assert!(!data.is_empty());
        assert_eq!(&data[0..4], b"RIFF", "Should be WebP (RIFF container)");
    }

    #[test]
    fn test_generate_thumbnail_resizes_large_image() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(256, 128, |_, _| {
            image::Rgba([100, 100, 100, 255])
        }));
        let data = generate_thumbnail(&img, 128).unwrap();
        assert!(!data.is_empty());
        assert_eq!(&data[0..4], b"RIFF");
    }

    #[test]
    fn test_generate_thumbnail_skips_resize_for_small_image() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(32, 32, |_, _| {
            image::Rgba([200, 200, 200, 255])
        }));
        let data = generate_thumbnail(&img, 128).unwrap();
        assert!(!data.is_empty());
    }

    // -----------------------------------------------------------------------
    // process_asset – audio
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_process_audio_extracts_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let db = create_test_db().await;
        let folder_path = dir.path().to_string_lossy().replace('\\', "/");
        let folder_id = insert_source_folder(&db, &folder_path, "test").await;
        create_test_wav(dir.path(), "test.wav");

        let mut asset = make_test_asset_with_path("test.wav", &folder_path, folder_id, "audio", "wav");
        asset.id = insert_asset(&db, &asset).await;

        let result = process_asset(&asset, &db).await;
        assert!(result.success, "Should succeed: {:?}", result.error);

        let (duration_ms, sample_rate, channels): (i64, Option<i32>, Option<i32>) =
            sqlx::query_as(
                "SELECT duration_ms, sample_rate, channels FROM audio_metadata WHERE asset_id = ?",
            )
            .bind(asset.id)
            .fetch_one(&db)
            .await
            .expect("Should have audio_metadata row");

        assert_eq!(sample_rate, Some(44100));
        assert_eq!(channels, Some(1));
        // 4410 samples @ 44100 Hz ≈ 100 ms
        assert!(
            duration_ms > 0,
            "Duration should be positive, got {}",
            duration_ms
        );
    }

    // -----------------------------------------------------------------------
    // process_asset – unsupported type
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_process_unsupported_asset_type() {
        let db = create_test_db().await;
        let folder_id = insert_source_folder(&db, "/fake", "fake").await;
        let mut asset = make_asset("test.mp4", folder_id, "", "video", "mp4");
        asset.id = 999; // not in DB, but we won't reach the DB write

        let result = process_asset(&asset, &db).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unsupported"));
    }

    // -----------------------------------------------------------------------
    // process_asset – missing file
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_process_image_missing_file_returns_error() {
        let db = create_test_db().await;
        let folder_id = insert_source_folder(&db, "/no/such", "missing").await;
        let mut asset = make_asset("gone.png", folder_id, "path", "image", "png");
        asset.folder_path = "/no/such".to_string();
        asset.id = insert_asset(&db, &asset).await;

        let result = process_asset(&asset, &db).await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
