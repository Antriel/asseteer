/// Batched database writer for asset processing results.
///
/// Instead of each worker doing individual INSERTs (causing ~500 transactions/sec
/// and heavy SQLite write lock contention), workers send results to a shared
/// `DbBatchWriter` that flushes them in batched transactions.
use crate::models::ProcessingCategory;
use crate::utils::unix_now;
use sqlx::sqlite::SqliteConnection;
use sqlx::SqlitePool;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, Duration};

const WRITE_BATCH_SIZE: usize = 64;
const FLUSH_INTERVAL_MS: u64 = 100;
const CHANNEL_CAPACITY: usize = 2048;

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
    tx: mpsc::Sender<WriterCommand>,
}

impl DbBatchWriter {
    /// Create a new batch writer that spawns a background writer task.
    pub fn new(db: SqlitePool) -> Self {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        tokio::spawn(writer_task(db, rx));
        Self { tx }
    }

    /// Send a processing result to be written in the next batch.
    pub async fn send(&self, item: ProcessingOutput) {
        let _ = self.tx.send(WriterCommand::Write(item)).await;
    }

    /// Flush all buffered writes and wait for the transaction to commit.
    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(WriterCommand::Flush(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }
}

/// Background task that receives write items and flushes them in batched transactions.
async fn writer_task(db: SqlitePool, mut rx: mpsc::Receiver<WriterCommand>) {
    let mut buffer: Vec<ProcessingOutput> = Vec::with_capacity(WRITE_BATCH_SIZE);
    let mut flush_waiters: Vec<oneshot::Sender<()>> = Vec::new();
    let mut tick = interval(Duration::from_millis(FLUSH_INTERVAL_MS));
    // First tick fires immediately — skip it so we don't flush an empty buffer
    tick.tick().await;

    loop {
        tokio::select! {
            biased;

            cmd = rx.recv() => {
                match cmd {
                    Some(WriterCommand::Write(item)) => {
                        buffer.push(item);
                        if buffer.len() >= WRITE_BATCH_SIZE {
                            flush_batch(&db, &mut buffer).await;
                            notify_waiters(&mut flush_waiters);
                        }
                    }
                    Some(WriterCommand::Flush(waiter)) => {
                        // Drain any remaining items from channel before flushing
                        drain_channel(&mut rx, &mut buffer, &mut flush_waiters);
                        if !buffer.is_empty() {
                            flush_batch(&db, &mut buffer).await;
                        }
                        let _ = waiter.send(());
                        notify_waiters(&mut flush_waiters);
                    }
                    None => {
                        // Channel closed — flush remaining items
                        if !buffer.is_empty() {
                            flush_batch(&db, &mut buffer).await;
                        }
                        notify_waiters(&mut flush_waiters);
                        break;
                    }
                }
            }
            _ = tick.tick() => {
                // Also drain any pending items before the periodic flush
                drain_channel(&mut rx, &mut buffer, &mut flush_waiters);
                if !buffer.is_empty() {
                    flush_batch(&db, &mut buffer).await;
                    notify_waiters(&mut flush_waiters);
                }
            }
        }
    }
}

/// Drain all immediately-available items from the channel into the buffer.
fn drain_channel(
    rx: &mut mpsc::Receiver<WriterCommand>,
    buffer: &mut Vec<ProcessingOutput>,
    flush_waiters: &mut Vec<oneshot::Sender<()>>,
) {
    loop {
        match rx.try_recv() {
            Ok(WriterCommand::Write(item)) => buffer.push(item),
            Ok(WriterCommand::Flush(waiter)) => flush_waiters.push(waiter),
            Err(_) => break,
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
async fn flush_batch(db: &SqlitePool, buffer: &mut Vec<ProcessingOutput>) {
    let items: Vec<ProcessingOutput> = buffer.drain(..).collect();
    let count = items.len();

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!(
                "[DbBatchWriter] Failed to begin transaction ({} items): {}",
                count, e
            );
            // Fall back to individual writes (each gets its own implicit transaction)
            for item in items {
                write_item_to_pool(db, item).await;
            }
            return;
        }
    };

    let now = unix_now();
    for item in items {
        write_item(&mut *tx, item, now).await;
    }

    if let Err(e) = tx.commit().await {
        eprintln!(
            "[DbBatchWriter] Failed to commit ({} items): {}",
            count, e
        );
    }
}

/// Execute a single write item within a transaction.
async fn write_item(conn: &mut SqliteConnection, item: ProcessingOutput, now: i64) {
    match item {
        ProcessingOutput::ImageSuccess {
            asset_id,
            width,
            height,
            thumbnail,
        } => {
            let _ = sqlx::query(
                "INSERT INTO image_metadata (asset_id, width, height, thumbnail_data, processed_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     width = excluded.width,
                     height = excluded.height,
                     processed_at = excluded.processed_at,
                     thumbnail_data = CASE
                         WHEN image_metadata.thumbnail_data IS NULL THEN excluded.thumbnail_data
                         ELSE image_metadata.thumbnail_data
                     END",
            )
            .bind(asset_id)
            .bind(width)
            .bind(height)
            .bind(thumbnail.as_deref())
            .bind(now)
            .execute(&mut *conn)
            .await;

            let _ = sqlx::query(
                "UPDATE processing_errors SET resolved_at = ? WHERE asset_id = ? AND resolved_at IS NULL",
            )
            .bind(now)
            .bind(asset_id)
            .execute(&mut *conn)
            .await;
        }
        ProcessingOutput::AudioSuccess {
            asset_id,
            duration_ms,
            sample_rate,
            channels,
        } => {
            let _ = sqlx::query(
                "INSERT INTO audio_metadata (asset_id, duration_ms, sample_rate, channels, processed_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (asset_id) DO UPDATE SET
                     duration_ms = excluded.duration_ms,
                     sample_rate = excluded.sample_rate,
                     channels = excluded.channels,
                     processed_at = excluded.processed_at",
            )
            .bind(asset_id)
            .bind(duration_ms)
            .bind(sample_rate)
            .bind(channels)
            .bind(now)
            .execute(&mut *conn)
            .await;

            let _ = sqlx::query(
                "UPDATE processing_errors SET resolved_at = ? WHERE asset_id = ? AND resolved_at IS NULL",
            )
            .bind(now)
            .bind(asset_id)
            .execute(&mut *conn)
            .await;
        }
        ProcessingOutput::Failure {
            asset_id,
            category,
            error,
        } => {
            let _ = sqlx::query(
                "INSERT INTO processing_errors (asset_id, category, error_message, occurred_at, retry_count)
                 VALUES (?, ?, ?, ?, 0)",
            )
            .bind(asset_id)
            .bind(category.as_str())
            .bind(&error)
            .bind(now)
            .execute(&mut *conn)
            .await;
        }
    }
}

/// Fallback: write a single item using the pool (each gets its own implicit transaction).
async fn write_item_to_pool(db: &SqlitePool, item: ProcessingOutput) {
    let mut conn = match db.acquire().await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("[DbBatchWriter] Failed to acquire connection: {}", e);
            return;
        }
    };
    write_item(&mut *conn, item, unix_now()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[tokio::test]
    async fn test_batch_writer_flushes_on_explicit_flush() {
        let db = create_test_db().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;

        for i in 0..3 {
            let asset = make_asset(&format!("img_{}.png", i), folder_id, "", "image", "png");
            insert_asset(&db, &asset).await;
        }

        let writer = DbBatchWriter::new(db.clone());

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
        let db = create_test_db().await;
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

        let writer = DbBatchWriter::new(db.clone());

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
        let db = create_test_db().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;
        let asset = make_asset("bad.png", folder_id, "", "image", "png");
        let id = insert_asset(&db, &asset).await;

        let writer = DbBatchWriter::new(db.clone());

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
        let db = create_test_db().await;
        let folder_id = insert_source_folder(&db, "/test", "test").await;
        let asset = make_asset("test.wav", folder_id, "", "audio", "wav");
        let id = insert_asset(&db, &asset).await;

        let writer = DbBatchWriter::new(db.clone());

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
