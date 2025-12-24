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

#[derive(Debug, Clone)]
pub struct Evidence {
    pub id: i64,
    pub concept: String,
    pub content: String,
    pub source: String,
    pub captured_at: String,
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub id: i64,
    pub concept: String,
    pub statement: String,
    pub evidence_id: Option<i64>,
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

            CREATE TABLE IF NOT EXISTS evidence (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              concept TEXT NOT NULL,
              content TEXT NOT NULL,
              source TEXT NOT NULL,
              captured_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS claims (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              concept TEXT NOT NULL,
              statement TEXT NOT NULL,
              evidence_id INTEGER,
              confidence REAL NOT NULL DEFAULT 0.3,
              created_at TEXT NOT NULL,
              FOREIGN KEY (evidence_id) REFERENCES evidence(id)
            );
            ",
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

    pub fn list_concept_names(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM concepts ORDER BY name ASC LIMIT ?1")?;
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
            ",
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
             LIMIT ?1",
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

    // --- Evidence ---
    pub fn add_evidence(&self, concept: &str, content: &str, source: &str) -> Result<i64> {
        let now = Self::now();
        self.conn.execute(
            "
            INSERT INTO evidence (concept, content, source, captured_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![concept, content, source, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_evidence_for(&self, concept: &str, limit: usize) -> Result<Vec<Evidence>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, concept, content, source, captured_at
            FROM evidence
            WHERE concept = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )?;

        let rows = stmt.query_map(params![concept, limit as i64], |row| {
            Ok(Evidence {
                id: row.get(0)?,
                concept: row.get(1)?,
                content: row.get(2)?,
                source: row.get(3)?,
                captured_at: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_evidence(&self, id: i64) -> Result<Option<Evidence>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, concept, content, source, captured_at
            FROM evidence
            WHERE id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Evidence {
                id: row.get(0)?,
                concept: row.get(1)?,
                content: row.get(2)?,
                source: row.get(3)?,
                captured_at: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    // --- Claims ---
    pub fn add_claim(
        &self,
        concept: &str,
        statement: &str,
        evidence_id: Option<i64>,
        confidence: f64,
    ) -> Result<i64> {
        let now = Self::now();
        self.conn.execute(
            "
            INSERT INTO claims (concept, statement, evidence_id, confidence, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![concept, statement, evidence_id, confidence, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_claims_for(&self, concept: &str, limit: usize) -> Result<Vec<Claim>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, concept, statement, evidence_id, confidence, created_at
            FROM claims
            WHERE concept = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )?;

        let rows = stmt.query_map(params![concept, limit as i64], |row| {
            Ok(Claim {
                id: row.get(0)?,
                concept: row.get(1)?,
                statement: row.get(2)?,
                evidence_id: row.get(3)?,
                confidence: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
