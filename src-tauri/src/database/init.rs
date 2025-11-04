use sqlx::SqlitePool;

use super::schema::*;

/// Migrate database to fix FTS table and triggers
pub async fn migrate_fts_triggers(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Drop old triggers
    sqlx::query("DROP TRIGGER IF EXISTS assets_ai")
        .execute(pool)
        .await?;
    sqlx::query("DROP TRIGGER IF EXISTS assets_au")
        .execute(pool)
        .await?;
    sqlx::query("DROP TRIGGER IF EXISTS assets_ad")
        .execute(pool)
        .await?;

    // Drop and recreate FTS table (to remove content=assets)
    sqlx::query("DROP TABLE IF EXISTS assets_fts")
        .execute(pool)
        .await?;

    // Recreate FTS table
    sqlx::query(CREATE_ASSETS_FTS)
        .execute(pool)
        .await?;

    // Recreate triggers
    for trigger_sql in CREATE_FTS_TRIGGERS.split("END;").filter(|s| !s.trim().is_empty()) {
        let trigger = format!("{} END;", trigger_sql.trim());
        sqlx::query(&trigger)
            .execute(pool)
            .await?;
    }

    // Repopulate FTS from existing assets
    sqlx::query(
        "INSERT INTO assets_fts(rowid, filename, path_segments)
         SELECT id, filename, REPLACE(path, '/', ' ') FROM assets"
    )
    .execute(pool)
    .await?;

    Ok(())
}

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
