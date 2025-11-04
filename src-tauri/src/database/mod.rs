pub mod init;
pub mod schema;

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

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

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&connection_string)
        .await?;

    // Run migrations/setup
    init::setup_database(&pool).await?;

    // Run migration to fix FTS triggers
    init::migrate_fts_triggers(&pool).await?;

    println!("[DB] Database pool initialized successfully");
    Ok(pool)
}

/// Close the database pool gracefully
pub async fn close_db(pool: DbPool) {
    println!("[DB] Closing database pool...");
    pool.close().await;
    println!("[DB] Database pool closed successfully");
}
