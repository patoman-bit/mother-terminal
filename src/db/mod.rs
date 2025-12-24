use rusqlite::{params, Connection, Result};
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

#[derive(Debug, Clone)]
pub struct Relation {
    pub id: i64,
    pub from: String,
    pub relation_type: String,
    pub to: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Episode {
    pub id: i64,
    pub captured_at: String,
    pub outcome: String, // "ok" | "fail" | "note"
    pub summary: String,
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

            CREATE TABLE IF NOT EXISTS concept_relations (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              from_concept TEXT NOT NULL,
              relation_type TEXT NOT NULL,
              to_concept TEXT NOT NULL,
              created_at TEXT NOT NULL,
              UNIQUE(from_concept, relation_type, to_concept)
            );

            CREATE TABLE IF NOT EXISTS episodes (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              captured_at TEXT NOT NULL,
              outcome TEXT NOT NULL,
              summary TEXT NOT NULL
            );
            "
        )?;

        Ok(Self { conn })
    }

    fn now() -> String {
        OffsetDateTime::now_utc().to_string()
    }

    // --- Concepts ---
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
            "SELECT id, name, definition, confidence, created_at FROM concepts WHERE name = ?1"
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
             LIMIT ?1"
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

    pub fn list_concept_names(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM concepts ORDER BY name ASC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| row.get(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // --- Relations ---
    pub fn upsert_relation(&self, from: &str, relation_type: &str, to: &str) -> Result<()> {
        let now = Self::now();
        self.conn.execute(
            "
            INSERT INTO concept_relations (from_concept, relation_type, to_concept, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(from_concept, relation_type, to_concept) DO NOTHING
            ",
            params![from, relation_type, to, now],
        )?;
        Ok(())
    }

    pub fn list_relations_for(&self, concept: &str, limit: usize) -> Result<Vec<Relation>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, from_concept, relation_type, to_concept, created_at
            FROM concept_relations
            WHERE from_concept = ?1 OR to_concept = ?1
            ORDER BY id DESC
            LIMIT ?2
            "
        )?;

        let rows = stmt.query_map(params![concept, limit as i64], |row| {
            Ok(Relation {
                id: row.get(0)?,
                from: row.get(1)?,
                relation_type: row.get(2)?,
                to: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // --- Episodes (experience) ---
    pub fn add_episode(&self, outcome: &str, summary: &str) -> Result<()> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO episodes (captured_at, outcome, summary) VALUES (?1, ?2, ?3)",
            params![now, outcome, summary],
        )?;
        Ok(())
    }

    pub fn list_episodes(&self, limit: usize) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, captured_at, outcome, summary
             FROM episodes
             ORDER BY id DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(Episode {
                id: row.get(0)?,
                captured_at: row.get(1)?,
                outcome: row.get(2)?,
                summary: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
