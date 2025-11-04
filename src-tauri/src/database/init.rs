use rusqlite::{Connection, Result};

use super::schema::*;

/// Create all database tables
pub fn create_tables(conn: &Connection) -> Result<()> {
    // Create main assets table
    conn.execute(CREATE_ASSETS_TABLE, [])?;

    // Create indexes
    conn.execute_batch(CREATE_ASSETS_INDEXES)?;

    // Create FTS5 virtual table
    conn.execute(CREATE_ASSETS_FTS, [])?;

    // Create FTS triggers
    conn.execute_batch(CREATE_FTS_TRIGGERS)?;

    // Create scan sessions table
    conn.execute(CREATE_SCAN_SESSIONS_TABLE, [])?;

    Ok(())
}

/// Configure SQLite for optimal performance
pub fn configure_sqlite(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-64000;
        PRAGMA temp_store=MEMORY;
        "
    )?;

    Ok(())
}
