use rusqlite::{Connection, Result, params};
use time::OffsetDateTime;

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct Concept {
    pub id: i64,
    pub name: String,
    pub definition: String,
    pub confidence: f64,
    pub created_at: String,
}

impl Database {
    pub fn init(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS concepts (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL UNIQUE,
              definition TEXT NOT NULL,
              confidence REAL NOT NULL DEFAULT 0.3,
              created_at TEXT NOT NULL
            );
            ",
        )?;

        Ok(Self { conn })
    }

    fn now() -> String {
        OffsetDateTime::now_utc().to_string()
    }

    pub fn upsert_concept(&self, name: &str, definition: &str, confidence: f64) -> Result<()> {
        let now = Self::now();
        self.conn.execute(
            "
            INSERT INTO concepts (name, definition, confidence, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(name) DO UPDATE SET
              definition = excluded.definition,
              confidence = excluded.confidence
            ",
            params![name, definition, confidence, now],
        )?;
        Ok(())
    }

    pub fn get_concept(&self, name: &str) -> Result<Option<Concept>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, definition, confidence, created_at FROM concepts WHERE name = ?1",
        )?;

        let mut rows = stmt.query(params![name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Concept {
                id: row.get(0)?,
                name: row.get(1)?,
                definition: row.get(2)?,
                confidence: row.get(3)?,
                created_at: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_concepts(&self, limit: usize) -> Result<Vec<Concept>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, definition, confidence, created_at
             FROM concepts
             ORDER BY id DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(Concept {
                id: row.get(0)?,
                name: row.get(1)?,
                definition: row.get(2)?,
                confidence: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
