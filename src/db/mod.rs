use rusqlite::{Connection, Result};

pub struct Database {
    _conn: Connection,
}

impl Database {
    pub fn init(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS concepts (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL UNIQUE,
              definition TEXT,
              confidence REAL NOT NULL DEFAULT 0.3,
              created_at TEXT NOT NULL
            );
            "
        )?;

        Ok(Self { _conn: conn })
    }
}
