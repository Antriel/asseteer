//! Shared test utilities: in-memory DB setup, fixture file creation, helpers.

use crate::database::init::setup_database;
use crate::models::Asset;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

/// Create an in-memory SQLite database with the full schema.
pub async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");

    setup_database(&pool)
        .await
        .expect("Failed to setup test database schema");

    pool
}

/// Create a file-backed SQLite database with WAL mode and the full schema.
/// Multiple pools can connect to the same file for concurrent access tests.
pub async fn create_test_db_file(path: &str) -> SqlitePool {
    let normalized = path.replace('\\', "/");
    let uri = format!("sqlite:///{}?mode=rwc", normalized);
    let opts = SqliteConnectOptions::from_str(&uri)
        .expect("Bad SQLite URI")
        .busy_timeout(Duration::from_secs(30));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("Failed to create file-backed test database");

    setup_database(&pool)
        .await
        .expect("Failed to setup test database schema");

    pool
}

/// Open a SECOND pool to an existing file-backed database (no schema setup).
/// Simulates the Tauri SQL plugin opening its own connection.
pub async fn open_reader_pool(path: &str) -> SqlitePool {
    let normalized = path.replace('\\', "/");
    let uri = format!("sqlite:///{}?mode=rwc", normalized);
    let opts = SqliteConnectOptions::from_str(&uri)
        .expect("Bad SQLite URI")
        .busy_timeout(Duration::from_secs(5));

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("Failed to open reader pool")
}

/// Insert a source folder and return its ID.
pub async fn insert_source_folder(pool: &SqlitePool, path: &str, label: &str) -> i64 {
    let result = sqlx::query(
        "INSERT INTO source_folders (path, label, added_at) VALUES (?, ?, 1000000)",
    )
    .bind(path)
    .bind(label)
    .execute(pool)
    .await
    .expect("Failed to insert test source folder");

    result.last_insert_rowid()
}

/// Build an Asset struct. ID will be 0; call `insert_asset` to get a real ID.
/// `folder_id` must reference an existing source_folder row.
pub fn make_asset(filename: &str, folder_id: i64, rel_path: &str, asset_type: &str, format: &str) -> Asset {
    Asset {
        id: 0,
        filename: filename.to_string(),
        folder_id,
        rel_path: rel_path.to_string(),
        zip_file: None,
        zip_entry: None,
        zip_compression: None,
        asset_type: asset_type.to_string(),
        format: format.to_string(),
        file_size: 1024,
        fs_modified_at: Some(1_000_000),
        created_at: 1_000_000,
        modified_at: 1_000_000,
        folder_path: String::new(),
    }
}

/// Insert an asset into the database and return the auto-assigned ID.
pub async fn insert_asset(pool: &SqlitePool, asset: &Asset) -> i64 {
    let result = sqlx::query(
        "INSERT INTO assets (filename, folder_id, rel_path, zip_file, zip_entry, asset_type, format, file_size, fs_modified_at, created_at, modified_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&asset.filename)
    .bind(asset.folder_id)
    .bind(&asset.rel_path)
    .bind(&asset.zip_file)
    .bind(&asset.zip_entry)
    .bind(&asset.asset_type)
    .bind(&asset.format)
    .bind(asset.file_size)
    .bind(asset.fs_modified_at)
    .bind(asset.created_at)
    .bind(asset.modified_at)
    .execute(pool)
    .await
    .expect("Failed to insert test asset");

    result.last_insert_rowid()
}

/// Create a 64x48 RGBA test PNG at `dir/<name>`.
pub fn create_test_png(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let img = image::RgbaImage::from_fn(64, 48, |x, y| {
        image::Rgba([x as u8, y as u8, 128, 255])
    });
    img.save(&path).expect("Failed to save test PNG");
    path
}

/// Create a minimal valid WAV file (0.1 s of mono silence, 44100 Hz, 16-bit PCM).
pub fn create_test_wav(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let sample_rate: u32 = 44100;
    let num_samples: u32 = 4410; // 0.1 s
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = num_samples * channels as u32 * bits_per_sample as u32 / 8;
    let file_size = 36 + data_size;

    let mut buf: Vec<u8> = Vec::with_capacity(44 + data_size as usize);
    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    buf.resize(44 + data_size as usize, 0); // silence

    std::fs::write(&path, &buf).expect("Failed to write test WAV");
    path
}

/// Create a ZIP containing a small test PNG. Returns (zip_path, entry_name).
pub fn create_test_zip_with_image(dir: &Path) -> (PathBuf, String) {
    let zip_path = dir.join("test_archive.zip");
    let entry_name = "images/test.png".to_string();

    // Generate PNG bytes in memory
    let img = image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([x as u8 * 8, y as u8 * 8, 64, 255])
    });
    let mut png_bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    )
    .expect("Failed to encode PNG");

    // Write ZIP
    let file = std::fs::File::create(&zip_path).expect("Failed to create ZIP file");
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file(&entry_name, options)
        .expect("Failed to start ZIP entry");
    zip.write_all(&png_bytes)
        .expect("Failed to write ZIP entry");
    zip.finish().expect("Failed to finish ZIP");

    (zip_path, entry_name)
}
