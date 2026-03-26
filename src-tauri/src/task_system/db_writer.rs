/// Batched database writer for asset processing results.
///
/// Workers send results to a shared `DbBatchWriter` which flushes them in batched
/// transactions via a synchronous rusqlite connection on `spawn_blocking`.
/// This mirrors the scan bulk-insert pattern: prepare once, rebind+step+reset in a
/// tight loop, with `wal_autocheckpoint=0` to avoid mid-processing checkpoint I/O.
use crate::models::ProcessingCategory;
use crate::utils::unix_now;
use crossbeam::channel::{self, Receiver, RecvTimeoutError, TryRecvError};
use rusqlite;
use std::time::Duration;
use tokio::sync::oneshot;

const WRITE_BATCH_SIZE: usize = 64;
const FLUSH_INTERVAL_MS: u64 = 100;

/// Result of CPU-only asset processing, ready to be written to the database.
pub enum ProcessingOutput {
    ImageSuccess {
        asset_id: i64,
        width: i32,
        height: i32,
        thumbnail: Option<Vec<u8>>,
    },
    AudioSuccess {
        asset_id: i64,
        duration_ms: i64,
        sample_rate: i32,
        channels: i32,
    },
    ClapSuccess {
        asset_id: i64,
        embedding: Vec<u8>,
    },
    Failure {
        asset_id: i64,
        category: ProcessingCategory,
        error: String,
    },
}

enum WriterCommand {
    Write(ProcessingOutput),
    Flush(oneshot::Sender<()>),
}

/// Cloneable handle to the batch writer. Workers send results here.
#[derive(Clone)]
pub struct DbBatchWriter {
    tx: channel::Sender<WriterCommand>,
}

impl DbBatchWriter {
    /// Create a new batch writer backed by a synchronous rusqlite connection.
    /// Spawns a blocking thread that owns the connection and processes writes.
    pub fn new(db_path: String) -> Self {
        let (tx, rx) = channel::unbounded();
        tokio::task::spawn_blocking(move || writer_task_sync(&db_path, rx));
        Self { tx }
    }

    /// Send a processing result to be written in the next batch.
    /// Non-blocking: crossbeam unbounded send is O(1).
    pub async fn send(&self, item: ProcessingOutput) {
        let _ = self.tx.send(WriterCommand::Write(item));
    }

    /// Flush all buffered writes and wait for the transaction to commit.
    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(WriterCommand::Flush(tx)).is_ok() {
            let _ = rx.await;
        }
    }
}

/// Synchronous writer task running on a blocking thread.
/// Opens its own rusqlite connection with WAL mode and disabled auto-checkpoint.
fn writer_task_sync(db_path: &str, rx: Receiver<WriterCommand>) {
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[DbBatchWriter] Failed to open database: {}", e);
            return;
        }
    };

    if let Err(e) = conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=30000; PRAGMA wal_autocheckpoint=0;",
    ) {
        eprintln!("[DbBatchWriter] Failed to set PRAGMAs: {}", e);
        return;
    }

    let mut buffer: Vec<ProcessingOutput> = Vec::with_capacity(WRITE_BATCH_SIZE);
    let mut flush_waiters: Vec<oneshot::Sender<()>> = Vec::new();

    loop {
        match rx.recv_timeout(Duration::from_millis(FLUSH_INTERVAL_MS)) {
            Ok(WriterCommand::Write(item)) => {
                buffer.push(item);
                if buffer.len() >= WRITE_BATCH_SIZE {
                    flush_batch_sync(&conn, &mut buffer);
                    notify_waiters(&mut flush_waiters);
                }
            }
            Ok(WriterCommand::Flush(waiter)) => {
                drain_channel_sync(&rx, &mut buffer, &mut flush_waiters);
                if !buffer.is_empty() {
                    flush_batch_sync(&conn, &mut buffer);
                }
                let _ = waiter.send(());
                notify_waiters(&mut flush_waiters);
            }
            Err(RecvTimeoutError::Timeout) => {
                drain_channel_sync(&rx, &mut buffer, &mut flush_waiters);
                if !buffer.is_empty() {
                    flush_batch_sync(&conn, &mut buffer);
                    notify_waiters(&mut flush_waiters);
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if !buffer.is_empty() {
                    flush_batch_sync(&conn, &mut buffer);
                }
                notify_waiters(&mut flush_waiters);
                break;
            }
        }
    }
}

/// Drain all immediately-available items from the channel into the buffer.
fn drain_channel_sync(
    rx: &Receiver<WriterCommand>,
    buffer: &mut Vec<ProcessingOutput>,
    flush_waiters: &mut Vec<oneshot::Sender<()>>,
) {
    loop {
        match rx.try_recv() {
            Ok(WriterCommand::Write(item)) => buffer.push(item),
            Ok(WriterCommand::Flush(waiter)) => flush_waiters.push(waiter),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
}

/// Notify all pending flush waiters.
fn notify_waiters(waiters: &mut Vec<oneshot::Sender<()>>) {
    for waiter in waiters.drain(..) {
        let _ = waiter.send(());
    }
}

/// Flush all buffered items in a single SQLite transaction.
fn flush_batch_sync(conn: &rusqlite::Connection, buffer: &mut Vec<ProcessingOutput>) {
    let items: Vec<ProcessingOutput> = buffer.drain(..).collect();
    let count = items.len();

    if let Err(e) = conn.execute_batch("BEGIN") {
        eprintln!(
            "[DbBatchWriter] Failed to begin transaction ({} items): {}",
            count, e
        );
        // Fall back to individual writes (each in its own implicit transaction)
        for item in items {
            write_item_sync(conn, item, unix_now());
        }
        return;
    }

    let now = unix_now();
    for item in items {
        write_item_sync(conn, item, now);
    }

    if let Err(e) = conn.execute_batch("COMMIT") {
        eprintln!(
            "[DbBatchWriter] Failed to commit ({} items): {}",
            count, e
        );
    }
}

/// Execute a single write item using cached prepared statements.
fn write_item_sync(conn: &rusqlite::Connection, item: ProcessingOutput, now: i64) {
    match item {
        ProcessingOutput::ImageSuccess {
            asset_id,
            width,
            height,
            thumbnail,
        } => {
            if let Ok(mut stmt) = conn.prepare_cached(
                "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     width = excluded.width,
                     height = excluded.height,
                     processed_at = excluded.processed_at,
                     thumbnail_data = CASE
                         WHEN image_metadata.thumbnail_data IS NULL THEN excluded.thumbnail_data
                         ELSE image_metadata.thumbnail_data
                     END",
            ) {
                let _ = stmt.execute(rusqlite::params![
                    asset_id,
                    width,
                    height,
                    thumbnail.as_deref(),
                    now,
                ]);
            }

            if let Ok(mut stmt) = conn.prepare_cached(
                "UPDATE processing_errors SET resolved_at = ?1 WHERE asset_id = ?2 AND resolved_at IS NULL",
            ) {
                let _ = stmt.execute(rusqlite::params![now, asset_id]);
            }
        }
        ProcessingOutput::AudioSuccess {
            asset_id,
            duration_ms,
            sample_rate,
            channels,
        } => {
            if let Ok(mut stmt) = conn.prepare_cached(
                "INSERT INTO audio_metadata (asset_id, duration_ms, sample_rate, channels, processed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     duration_ms = excluded.duration_ms,
                     sample_rate = excluded.sample_rate,
                     channels = excluded.channels,
                     processed_at = excluded.processed_at",
            ) {
                let _ = stmt.execute(rusqlite::params![
                    asset_id, duration_ms, sample_rate, channels, now,
                ]);
            }

            if let Ok(mut stmt) = conn.prepare_cached(
                "UPDATE processing_errors SET resolved_at = ?1 WHERE asset_id = ?2 AND resolved_at IS NULL",
            ) {
                let _ = stmt.execute(rusqlite::params![now, asset_id]);
            }
        }
        ProcessingOutput::ClapSuccess { asset_id, embedding } => {
            if let Ok(mut stmt) = conn.prepare_cached(
                "INSERT INTO audio_embeddings (asset_id, embedding, created_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     embedding = excluded.embedding,
                     created_at = excluded.created_at",
            ) {
                let _ = stmt.execute(rusqlite::params![asset_id, embedding, now]);
            }

            if let Ok(mut stmt) = conn.prepare_cached(
                "UPDATE processing_errors
                 SET resolved_at = ?1
                 WHERE asset_id = ?2 AND category = 'clap' AND resolved_at IS NULL",
            ) {
                let _ = stmt.execute(rusqlite::params![now, asset_id]);
            }
        }
        ProcessingOutput::Failure {
            asset_id,
            category,
            error,
        } => {
            if let Ok(mut stmt) = conn.prepare_cached(
                "INSERT INTO processing_errors (asset_id, category, error_message, occurred_at, retry_count)
                 VALUES (?1, ?2, ?3, ?4, 0)",
            ) {
                let _ = stmt.execute(rusqlite::params![
                    asset_id,
                    category.as_str(),
                    &error,
                    now,
                ]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::time::Duration;

    /// Create a file-backed test DB and return pool, path, and tempdir handle.
    /// The tempdir handle must be kept alive for the duration of the test.
    async fn test_db_with_path() -> (sqlx::SqlitePool, String, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let path_str = path.to_str().unwrap().to_string();
        let pool = create_test_db_file(&path_str).await;
        (pool, path_str, dir)
    }

    #[tokio::test]
    async fn test_batch_writer_flushes_on_explicit_flush() {
        let (db, db_path, _dir) = test_db_with_path().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;

        for i in 0..3 {
            let asset = make_asset(&format!("img_{}.png", i), folder_id, "", "image", "png");
            insert_asset(&db, &asset).await;
        }

        let writer = DbBatchWriter::new(db_path);

        for i in 1..=3i64 {
            writer
                .send(ProcessingOutput::ImageSuccess {
                    asset_id: i,
                    width: 100,
                    height: 200,
                    thumbnail: None,
                })
                .await;
        }

        writer.flush().await;

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM image_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_batch_writer_flushes_on_batch_size() {
        let (db, db_path, _dir) = test_db_with_path().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;

        for i in 0..WRITE_BATCH_SIZE {
            let asset = make_asset(
                &format!("img_{}.png", i),
                folder_id,
                "",
                "image",
                "png",
            );
            insert_asset(&db, &asset).await;
        }

        let writer = DbBatchWriter::new(db_path);

        for i in 1..=(WRITE_BATCH_SIZE as i64) {
            writer
                .send(ProcessingOutput::ImageSuccess {
                    asset_id: i,
                    width: 64,
                    height: 48,
                    thumbnail: None,
                })
                .await;
        }

        // Give the writer task time to process the batch-size trigger
        tokio::time::sleep(Duration::from_millis(50)).await;
        writer.flush().await;

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM image_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, WRITE_BATCH_SIZE as i64);
    }

    #[tokio::test]
    async fn test_batch_writer_handles_errors() {
        let (db, db_path, _dir) = test_db_with_path().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;
        let asset = make_asset("bad.png", folder_id, "", "image", "png");
        let id = insert_asset(&db, &asset).await;

        let writer = DbBatchWriter::new(db_path);

        writer
            .send(ProcessingOutput::Failure {
                asset_id: id,
                category: ProcessingCategory::Image,
                error: "test error".to_string(),
            })
            .await;

        writer.flush().await;

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM processing_errors")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_batch_writer_handles_audio() {
        let (db, db_path, _dir) = test_db_with_path().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;
        let asset = make_asset("test.wav", folder_id, "", "audio", "wav");
        let id = insert_asset(&db, &asset).await;

        let writer = DbBatchWriter::new(db_path);

        writer
            .send(ProcessingOutput::AudioSuccess {
                asset_id: id,
                duration_ms: 5000,
                sample_rate: 44100,
                channels: 2,
            })
            .await;

        writer.flush().await;

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audio_metadata")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
