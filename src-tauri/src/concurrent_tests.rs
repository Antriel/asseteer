//! Tests that verify the frontend SQL reads don't hang when the backend is
//! writing (thumbnails, processing, etc.).  Uses a file-backed SQLite DB with
//! two separate connection pools to simulate the real dual-access pattern.

use crate::test_helpers::*;
use sqlx::SqlitePool;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Seed the database with `n` image assets (plus image_metadata rows without
/// thumbnails, mimicking post-processing state after lazy-load rework).
async fn seed_images(pool: &SqlitePool, n: usize) {
    for i in 0..n {
        let filename = format!("image_{:05}.png", i);
        let path = format!("/test/images/{}", filename);

        let id = sqlx::query(
            "INSERT INTO assets (filename, path, asset_type, format, file_size, created_at, modified_at)
             VALUES (?, ?, 'image', 'png', 1024, 1000000, 1000000)",
        )
        .bind(&filename)
        .bind(&path)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid();

        // image_metadata with NULL thumbnail (the lazy-load state)
        sqlx::query(
            "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
             VALUES (?, 64, 48, NULL, 1000000)",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify the search query returns results on a clean database.
#[tokio::test]
async fn test_search_query_returns_results() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("search_test.db");
    let pool = create_test_db_file(db_path.to_str().unwrap()).await;

    seed_images(&pool, 20).await;

    let rows: Vec<(i64,)> = sqlx::query_as("SELECT id FROM assets WHERE asset_type = 'image'")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 20);

    // FTS search — filenames contain "image" so "image*" should match all
    let results: Vec<(i64, String)> =
        sqlx::query_as("SELECT assets.id, assets.filename FROM assets INNER JOIN assets_fts ON assets.id = assets_fts.rowid WHERE assets_fts MATCH 'image*' LIMIT 50")
            .fetch_all(&pool)
            .await
            .unwrap();

    assert!(!results.is_empty(), "FTS search should return results");
}

/// Core test: a reader pool (simulating the Tauri SQL plugin) must be able to
/// execute the search query while a writer pool (simulating ensure_thumbnails)
/// is continuously updating image_metadata rows.
#[tokio::test]
async fn test_search_not_blocked_by_thumbnail_writes() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("concurrent_test.db");
    let db_str = db_path.to_str().unwrap();

    // Backend pool (sqlx — writes thumbnails)
    let backend = create_test_db_file(db_str).await;
    seed_images(&backend, 200).await;

    // Reader pool (simulates SQL plugin — runs search queries)
    let reader = open_reader_pool(db_str).await;

    // Spawn a writer that continuously updates thumbnails (like ensure_thumbnails)
    let writer = backend.clone();
    let write_handle = tokio::spawn(async move {
        let fake_thumb = vec![0u8; 5_000]; // ~5 KB thumbnail
        for i in 1i64..=200 {
            let _ = sqlx::query(
                "UPDATE image_metadata SET thumbnail_data = ? WHERE asset_id = ?",
            )
            .bind(&fake_thumb)
            .bind(i)
            .execute(&writer)
            .await;
            // Small delay to spread writes over time
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });

    // Give the writer a head start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Run the search query on the reader pool — should NOT be blocked
    let search_result = tokio::time::timeout(Duration::from_secs(5), async {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT assets.id, assets.filename
             FROM assets
             INNER JOIN assets_fts ON assets.id = assets_fts.rowid
             LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
             LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
             WHERE assets_fts MATCH 'image*' AND assets.asset_type = 'image'
             ORDER BY assets.filename COLLATE NOCASE ASC
             LIMIT 5001 OFFSET 0",
        )
        .fetch_all(&reader)
        .await
    })
    .await;

    // Clean up writer
    let _ = write_handle.await;

    match search_result {
        Ok(Ok(rows)) => {
            assert!(!rows.is_empty(), "Search should return results");
            println!("Search returned {} rows while writes were running", rows.len());
        }
        Ok(Err(e)) => panic!("Search query failed with DB error: {}", e),
        Err(_) => panic!("DEADLOCK: Search query did not complete within 5 seconds while thumbnail writes were running"),
    }
}

/// Same test but with the reader pool having NO busy_timeout — simulating the
/// Tauri SQL plugin's default connection which may not set busy_timeout.
#[tokio::test]
async fn test_search_not_blocked_without_busy_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("no_timeout_test.db");
    let db_str = db_path.to_str().unwrap();

    let backend = create_test_db_file(db_str).await;
    seed_images(&backend, 100).await;

    // Reader with NO busy timeout (0 = return SQLITE_BUSY immediately)
    let normalized = db_str.replace('\\', "/");
    let uri = format!("sqlite:///{}?mode=rwc", normalized);
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&uri)
        .unwrap()
        .busy_timeout(Duration::from_secs(0));

    let reader = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    // Writer continuously updating
    let writer = backend.clone();
    let write_handle = tokio::spawn(async move {
        let fake_thumb = vec![0u8; 5_000];
        for i in 1i64..=100 {
            let _ = sqlx::query(
                "UPDATE image_metadata SET thumbnail_data = ? WHERE asset_id = ?",
            )
            .bind(&fake_thumb)
            .bind(i)
            .execute(&writer)
            .await;
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    });

    tokio::time::sleep(Duration::from_millis(20)).await;

    // Try to read — might get SQLITE_BUSY
    let search_result = tokio::time::timeout(Duration::from_secs(5), async {
        // Retry up to 3 times on SQLITE_BUSY
        for attempt in 0..3 {
            match sqlx::query_as::<_, (i64,)>(
                "SELECT assets.id FROM assets
                 INNER JOIN assets_fts ON assets.id = assets_fts.rowid
                 WHERE assets_fts MATCH 'image*' AND assets.asset_type = 'image'
                 LIMIT 50",
            )
            .fetch_all(&reader)
            .await
            {
                Ok(rows) => return Ok(rows),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("busy") || msg.contains("locked") {
                        println!("Attempt {}: got SQLITE_BUSY, retrying...", attempt + 1);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        // Last attempt
        sqlx::query_as::<_, (i64,)>(
            "SELECT assets.id FROM assets
             INNER JOIN assets_fts ON assets.id = assets_fts.rowid
             WHERE assets_fts MATCH 'image*' AND assets.asset_type = 'image'
             LIMIT 50",
        )
        .fetch_all(&reader)
        .await
    })
    .await;

    let _ = write_handle.await;

    match search_result {
        Ok(Ok(rows)) => {
            println!("Search returned {} rows (no busy_timeout reader)", rows.len());
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("busy") || msg.contains("locked") {
                panic!(
                    "SQLITE_BUSY: Reader cannot query while writer is active. \
                     The Tauri SQL plugin likely needs busy_timeout or WAL mode verification. \
                     Error: {}",
                    msg
                );
            }
            panic!("Unexpected DB error: {}", e);
        }
        Err(_) => panic!("DEADLOCK: Search query timed out with no busy_timeout"),
    }
}

/// Verify that the FTS search query performs reasonably even with many assets.
#[tokio::test]
async fn test_fts_search_performance() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("perf_test.db");
    let pool = create_test_db_file(db_path.to_str().unwrap()).await;

    // Insert 5000 assets
    seed_images(&pool, 5000).await;

    let start = std::time::Instant::now();
    let results: Vec<(i64,)> = sqlx::query_as(
        "SELECT assets.id
         FROM assets
         INNER JOIN assets_fts ON assets.id = assets_fts.rowid
         LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
         LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
         WHERE assets_fts MATCH 'image*' AND assets.asset_type = 'image'
         ORDER BY assets.filename COLLATE NOCASE ASC
         LIMIT 5001 OFFSET 0",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let elapsed = start.elapsed();

    println!(
        "FTS search over 5000 assets: {} results in {:?}",
        results.len(),
        elapsed
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "FTS search should complete in under 5 seconds, took {:?}",
        elapsed
    );
}

use std::str::FromStr;
