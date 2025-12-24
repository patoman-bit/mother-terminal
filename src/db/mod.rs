use rusqlite::{Connection, Result, params};
use std::collections::HashMap;
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
pub struct Evidence {
    pub id: i64,
    pub concept_name: String,
    pub content: String,
    pub source: Option<String>,
    pub domain: Option<String>,
    pub trust: f64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct EpisodeTag {
    pub episode_id: i64,
    pub concept_name: String,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SkillStep {
    pub id: i64,
    pub skill_id: i64,
    pub step_no: i64,
    pub text: String,
    pub evidence_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ConfidenceUpdate {
    pub concept: String,
    pub old: f64,
    pub new: f64,
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

            CREATE TABLE IF NOT EXISTS episode_tags (
              episode_id INTEGER NOT NULL,
              concept_name TEXT NOT NULL,
              UNIQUE(episode_id, concept_name)
            );

            CREATE TABLE IF NOT EXISTS evidence (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              concept_name TEXT NOT NULL,
              content TEXT NOT NULL,
              source TEXT,
              domain TEXT,
              trust REAL NOT NULL DEFAULT 0.50,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS concept_confidence_events (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              concept_name TEXT NOT NULL,
              event_type TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skills (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL UNIQUE,
              description TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skill_steps (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              skill_id INTEGER NOT NULL,
              step_no INTEGER NOT NULL,
              text TEXT NOT NULL,
              evidence_id INTEGER,
              episode_id INTEGER,
              created_at TEXT NOT NULL
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

    pub fn record_confidence_event(&self, concept: &str, event_type: &str) -> Result<()> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO concept_confidence_events (concept_name, event_type, created_at) VALUES (?1, ?2, ?3)",
            params![concept, event_type, now],
        )?;
        Ok(())
    }

    fn concept_event_counts(&self) -> Result<HashMap<String, HashMap<String, i64>>> {
        let mut stmt = self.conn.prepare(
            "SELECT concept_name, event_type, COUNT(*) FROM concept_confidence_events GROUP BY concept_name, event_type"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut map: HashMap<String, HashMap<String, i64>> = HashMap::new();
        for r in rows {
            let (concept, event, count) = r?;
            map.entry(concept).or_default().insert(event, count);
        }
        Ok(map)
    }

    pub fn calculate_confidence_updates(&self) -> Result<Vec<ConfidenceUpdate>> {
        let concepts = self.list_concepts(10_000)?;
        let event_counts = self.concept_event_counts()?;
        let mut stmt = self
            .conn
            .prepare("SELECT concept_name, AVG(trust) FROM evidence GROUP BY concept_name")?;
        let mut trust_map: HashMap<String, f64> = HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        for r in rows {
            let (concept, avg) = r?;
            trust_map.insert(concept, avg);
        }

        let mut updates = Vec::new();
        for c in concepts {
            let events = event_counts.get(&c.name).cloned().unwrap_or_default();
            let confirmed = *events.get("confirm_claim").unwrap_or(&0) as f64;
            let rejected = *events.get("reject_claim").unwrap_or(&0) as f64;
            let ep_ok = *events.get("episode_ok").unwrap_or(&0) as f64;
            let ep_fail = *events.get("episode_fail").unwrap_or(&0) as f64;
            let avg_trust = trust_map.get(&c.name).cloned().unwrap_or(0.5);

            // Confidence evolution rules (deterministic):
            // base 0.30, confirmed claims push up, rejected push down,
            // positive/negative episodes nudge, trust influences mildly.
            let mut new_conf = 0.30 + 0.15 * confirmed - 0.12 * rejected + 0.08 * ep_ok
                - 0.08 * ep_fail
                + 0.25 * (avg_trust - 0.5);

            if new_conf > 1.0 {
                new_conf = 1.0;
            }
            if new_conf < 0.0 {
                new_conf = 0.0;
            }

            if (new_conf - c.confidence).abs() > f64::EPSILON {
                updates.push(ConfidenceUpdate {
                    concept: c.name,
                    old: c.confidence,
                    new: new_conf,
                });
            }
        }
        Ok(updates)
    }

    pub fn apply_confidence_updates(&self, updates: &[ConfidenceUpdate]) -> Result<()> {
        for u in updates {
            self.conn.execute(
                "UPDATE concepts SET confidence = ?1 WHERE name = ?2",
                params![u.new, u.concept],
            )?;
        }
        Ok(())
    }

    pub fn concept_confidence_history(&self, concept: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT event_type, created_at FROM concept_confidence_events WHERE concept_name = ?1 ORDER BY id DESC LIMIT 50"
        )?;
        let rows = stmt.query_map(params![concept], |row| Ok((row.get(0)?, row.get(1)?)))?;
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

    pub fn list_all_relations(&self, limit: usize) -> Result<Vec<Relation>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, from_concept, relation_type, to_concept, created_at
            FROM concept_relations
            ORDER BY id DESC
            LIMIT ?1
            ",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
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
    pub fn add_episode(&self, outcome: &str, summary: &str) -> Result<i64> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO episodes (captured_at, outcome, summary) VALUES (?1, ?2, ?3)",
            params![now, outcome, summary],
        )?;
        Ok(self.conn.last_insert_rowid())
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

    pub fn get_episode(&self, id: i64) -> Result<Option<Episode>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, captured_at, outcome, summary FROM episodes WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Episode {
                id: row.get(0)?,
                captured_at: row.get(1)?,
                outcome: row.get(2)?,
                summary: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn add_episode_tag(&self, episode_id: i64, concept_name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO episode_tags (episode_id, concept_name) VALUES (?1, ?2)",
            params![episode_id, concept_name],
        )?;
        Ok(())
    }

    pub fn list_episode_tags(&self, episode_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT concept_name FROM episode_tags WHERE episode_id = ?1 ORDER BY concept_name ASC",
        )?;
        let rows = stmt.query_map(params![episode_id], |row| row.get(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_episodes_for_concept(&self, concept: &str, limit: usize) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT e.id, e.captured_at, e.outcome, e.summary
            FROM episodes e
            JOIN episode_tags t ON e.id = t.episode_id
            WHERE t.concept_name = ?1
            ORDER BY e.id DESC
            LIMIT ?2
            ",
        )?;

        let rows = stmt.query_map(params![concept, limit as i64], |row| {
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
    pub fn add_evidence(
        &self,
        concept_name: &str,
        content: &str,
        source: Option<String>,
        domain: Option<String>,
    ) -> Result<i64> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO evidence (concept_name, content, source, domain, trust, created_at) VALUES (?1, ?2, ?3, ?4, 0.50, ?5)",
            params![concept_name, content, source, domain, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_evidence_for(&self, concept_name: &str, limit: usize) -> Result<Vec<Evidence>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, concept_name, content, source, domain, trust, created_at
            FROM evidence
            WHERE concept_name = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![concept_name, limit as i64], |row| {
            Ok(Evidence {
                id: row.get(0)?,
                concept_name: row.get(1)?,
                content: row.get(2)?,
                source: row.get(3)?,
                domain: row.get(4)?,
                trust: row.get(5)?,
                created_at: row.get(6)?,
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
            "SELECT id, concept_name, content, source, domain, trust, created_at FROM evidence WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Evidence {
                id: row.get(0)?,
                concept_name: row.get(1)?,
                content: row.get(2)?,
                source: row.get(3)?,
                domain: row.get(4)?,
                trust: row.get(5)?,
                created_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn adjust_trust(&self, id: i64, direction: &str) -> Result<Option<Evidence>> {
        let ev = match self.get_evidence(id)? {
            Some(e) => e,
            None => return Ok(None),
        };
        let mut trust = ev.trust;
        match direction {
            "up" => trust = (trust + 0.1).min(1.0),
            "down" => trust = (trust - 0.1).max(0.0),
            _ => {}
        }
        self.conn.execute(
            "UPDATE evidence SET trust = ?1 WHERE id = ?2",
            params![trust, id],
        )?;
        self.get_evidence(id)
    }

    // --- Skills ---
    pub fn add_skill(&self, name: &str, description: &str) -> Result<i64> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO skills (name, description, created_at) VALUES (?1, ?2, ?3)",
            params![name, description, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_skill(&self, name: &str) -> Result<Option<Skill>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, description, created_at FROM skills WHERE name = ?1")?;
        let mut rows = stmt.query(params![name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_skills(&self, limit: usize) -> Result<Vec<Skill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, created_at FROM skills ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn add_skill_step(
        &self,
        skill_id: i64,
        step_no: i64,
        text: &str,
        evidence_id: Option<i64>,
        episode_id: Option<i64>,
    ) -> Result<i64> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO skill_steps (skill_id, step_no, text, evidence_id, episode_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![skill_id, step_no, text, evidence_id, episode_id, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_skill_steps(&self, skill_id: i64) -> Result<Vec<SkillStep>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, skill_id, step_no, text, evidence_id, episode_id, created_at FROM skill_steps WHERE skill_id = ?1 ORDER BY step_no ASC"
        )?;
        let rows = stmt.query_map(params![skill_id], |row| {
            Ok(SkillStep {
                id: row.get(0)?,
                skill_id: row.get(1)?,
                step_no: row.get(2)?,
                text: row.get(3)?,
                evidence_id: row.get(4)?,
                episode_id: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
