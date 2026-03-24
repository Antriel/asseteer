pub mod init;
pub mod schema;

use sqlx::{Executor, SqlitePool, sqlite::{SqlitePoolOptions, SqliteConnectOptions}};
use std::str::FromStr;
use std::time::Duration;

pub type DbPool = SqlitePool;

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
pub async fn checkpoint_truncate(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;
    Ok(())
}

/// Close the database pool gracefully
pub async fn close_db(pool: DbPool) {
    println!("[DB] Closing database pool...");
    pool.close().await;
    println!("[DB] Database pool closed successfully");
}
