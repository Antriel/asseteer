use sqlx::SqlitePool;

use super::schema::*;

/// Setup database: configure SQLite and create tables
pub async fn setup_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Configure SQLite for optimal performance
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA synchronous=NORMAL")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA cache_size=-64000")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA temp_store=MEMORY")
        .execute(pool)
        .await?;

    // Create main assets table
    sqlx::query(CREATE_ASSETS_TABLE)
        .execute(pool)
        .await?;

    // Create indexes
    for index_sql in CREATE_ASSETS_INDEXES.split(';').filter(|s| !s.trim().is_empty()) {
        sqlx::query(index_sql.trim())
            .execute(pool)
            .await?;
    }

    // Create FTS5 virtual table
    sqlx::query(CREATE_ASSETS_FTS)
        .execute(pool)
        .await?;

    // Create FTS triggers
    for trigger_sql in CREATE_FTS_TRIGGERS.split("END;").filter(|s| !s.trim().is_empty()) {
        let trigger = format!("{} END;", trigger_sql.trim());
        sqlx::query(&trigger)
            .execute(pool)
            .await?;
    }

    // Create scan sessions table
    sqlx::query(CREATE_SCAN_SESSIONS_TABLE)
        .execute(pool)
        .await?;

    Ok(())
}
