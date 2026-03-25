pub mod init;
pub mod schema;

use sqlx::{Executor, SqlitePool, sqlite::{SqlitePoolOptions, SqliteConnectOptions}};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;

pub type DbPool = SqlitePool;

const CHECKPOINT_MAX_ATTEMPTS: u32 = 10;
const CHECKPOINT_RETRY_DELAY_MS: u64 = 250;

#[derive(Debug, Clone, Copy)]
pub struct WalCheckpointResult {
    pub busy: bool,
    pub wal_pages_before: i64,
    pub wal_pages: i64,
    pub attempts: u32,
}

/// Initialize the database pool
pub async fn initialize_db(db_path: &str) -> Result<DbPool, sqlx::Error> {
    println!("[DB] Initializing database at: {}", db_path);

    // Create connection pool
    // Convert Windows backslashes to forward slashes for URI
    let normalized_path = db_path.replace('\\', "/");

    // For absolute paths, use sqlite:/// (three slashes)
    // For relative paths, use sqlite:// (two slashes)
    let connection_string = if normalized_path.contains(':') {
        // Absolute path (Windows: C:/... or Unix: /...)
        format!("sqlite:///{}?mode=rwc", normalized_path)
    } else {
        // Relative path
        format!("sqlite://{}?mode=rwc", normalized_path)
    };

    println!("[DB] Connection string: {}", connection_string);

    // Configure SQLite connection options with busy timeout
    let connect_options = SqliteConnectOptions::from_str(&connection_string)?
        .busy_timeout(Duration::from_secs(30)); // 30 second busy timeout for concurrent writes

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // Auto-checkpoint at 50000 pages (~200MB of WAL) on backend connections.
                // This bounds WAL growth during FTS population (which commits per batch)
                // without checkpointing too aggressively during bulk writes.
                // The frontend connection keeps auto-checkpoint disabled separately
                // to avoid adding checkpoint I/O latency to UI queries.
                conn.execute("PRAGMA wal_autocheckpoint=50000").await?;
                Ok(())
            })
        })
        .connect_with(connect_options)
        .await?;

    // Run migrations/setup
    init::setup_database(&pool).await?;

    println!("[DB] Database pool initialized successfully");
    Ok(pool)
}

/// Run a truncate WAL checkpoint. Waits for readers to finish, then
/// checkpoints all frames and truncates the WAL file to zero bytes.
/// Safe to call after bulk operations when no more writes are expected.
///
/// SQLite reports checkpoint completion via the returned row, not only via SQL
/// errors. A TRUNCATE checkpoint can still return BUSY if readers keep the WAL
/// pinned, so retry briefly before giving up.
pub async fn checkpoint_truncate(pool: &DbPool) -> Result<WalCheckpointResult, sqlx::Error> {
    let mut last_result = None;

    for attempt in 1..=CHECKPOINT_MAX_ATTEMPTS {
        let (_noop_busy, wal_pages_before, _checkpointed_pages_before): (i64, i64, i64) =
            sqlx::query_as("PRAGMA wal_checkpoint(NOOP)")
                .fetch_one(pool)
                .await?;

        let (busy, wal_pages, _checkpointed_pages): (i64, i64, i64) =
            sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
                .fetch_one(pool)
                .await?;

        let result = WalCheckpointResult {
            busy: busy != 0,
            wal_pages_before,
            wal_pages,
            attempts: attempt,
        };

        if !result.busy {
            println!(
                "[DB] WAL checkpoint(TRUNCATE) complete on attempt {}: WAL before={} pages, after={} pages",
                result.attempts,
                result.wal_pages_before,
                result.wal_pages
            );
            return Ok(result);
        }

        last_result = Some(result);

        if attempt < CHECKPOINT_MAX_ATTEMPTS {
            eprintln!(
                "[DB] WAL checkpoint(TRUNCATE) busy on attempt {}/{}: WAL before={} pages, after={} pages, retrying in {}ms",
                attempt,
                CHECKPOINT_MAX_ATTEMPTS,
                result.wal_pages_before,
                result.wal_pages,
                CHECKPOINT_RETRY_DELAY_MS
            );
            sleep(Duration::from_millis(CHECKPOINT_RETRY_DELAY_MS)).await;
        }
    }

    let result = last_result.expect("checkpoint loop must run at least once");
    eprintln!(
        "[DB] WAL checkpoint(TRUNCATE) still busy after {} attempts: WAL before={} pages, after={} pages",
        result.attempts,
        result.wal_pages_before,
        result.wal_pages
    );
    Ok(result)
}

/// Close the database pool gracefully
pub async fn close_db(pool: DbPool) {
    println!("[DB] Closing database pool...");
    pool.close().await;
    println!("[DB] Database pool closed successfully");
}
