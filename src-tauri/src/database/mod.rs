pub mod init;
pub mod schema;

use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

pub type DbConnection = Arc<Mutex<Connection>>;

/// Initialize the database and return a thread-safe connection
pub fn initialize_db(db_path: &str) -> Result<DbConnection> {
    let conn = Connection::open(db_path)?;
    init::create_tables(&conn)?;
    init::configure_sqlite(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}
