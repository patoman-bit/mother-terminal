use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use super::Module;
use crate::db::{Concept, Database};

#[derive(Clone, Debug)]
enum PendingAction {
    Concept {
        name: String,
        definition: String,
        confidence: f64,
    },
    Relation {
        from: String,
        relation_type: String,
        to: String,
    },
    Episode {
        outcome: String,
        summary: String,
    },
    Evidence {
        source: String,
        excerpt: String,
        trust: f64,
    },
    Claim {
        concept: String,
        claim_text: String,
        evidence_id: Option<i64>,
        confidence: f64,
    },
    SearchPlan {
        query: String,
    },
    StoreEvidence {
        items: Vec<EvidenceCandidate>,
    },
}

#[derive(Clone, Debug)]
struct EvidenceCandidate {
    source: String,
    excerpt: String,
}

pub struct Dialog {
    input: String,
    history: Vec<String>,
    db: Database,
    pending: Option<PendingAction>,
    search_results: Vec<EvidenceCandidate>,
}

impl Dialog {
    pub fn new(db: Database) -> Self {
        Self {
            input: String::new(),
            history: vec![
                "MOTHER: DIALOG READY.".into(),
                "MOTHER: Commands:".into(),
                "  learn <concept> is <definition>".into(),
                "  rel <from> <type> <to>".into(),
                "  ep ok|fail|note <summary>".into(),
                "  episodes".into(),
                "  show <concept> | list".into(),
                "  src <url> :: <excerpt>".into(),
                "  claim <concept> :: <claim text> :: <evidence_id optional>".into(),
                "  claims <concept> | evidence".into(),
                "  search <query> | keep <n>/keep all".into(),
                "  doctor (tool readiness)".into(),
                "MOTHER: Any writes need approval. Press [y] to confirm, [n] to reject.".into(),
            ],
            db,
            pending: None,
            search_results: Vec::new(),
        }
    }

    fn push(&mut self, line: impl Into<String>) {
        self.history.push(line.into());
        if self.history.len() > 240 {
            self.history.drain(0..70);
        }
    }

    fn eliza_reflect(&self, text: &str) -> String {
        format!("MOTHER: Why do you say '{}'?", &text)
    }

    fn handle_command(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        if let Some(_) = &self.pending {
            if trimmed.eq_ignore_ascii_case("y") {
                self.confirm_pending();
                return;
            } else if trimmed.eq_ignore_ascii_case("n") {
                self.reject_pending();
                return;
            }
            self.push("MOTHER: Respond [y]/[n] or ESC to cancel pending action before issuing new commands.");
            return;
        }

        // episodes list
        if trimmed.eq_ignore_ascii_case("episodes") {
            match self.db.list_episodes(20) {
                Ok(items) if items.is_empty() => self.push("MOTHER: No episodes stored yet."),
                Ok(items) => {
                    self.push("MOTHER: Recent episodes:");
                    for e in items {
                        self.push(format!(
                            "  - [{}] {}  {}",
                            e.outcome, e.captured_at, e.summary
                        ));
                    }
                }
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // list concepts
        if trimmed.eq_ignore_ascii_case("list") {
            match self.db.list_concepts(20) {
                Ok(items) if items.is_empty() => self.push("MOTHER: No concepts stored yet."),
                Ok(items) => {
                    self.push("MOTHER: Recent concepts:");
                    for c in items {
                        self.push(format!("  - {} (conf {:.2})", c.name, c.confidence));
                    }
                }
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // evidence list
        if trimmed.eq_ignore_ascii_case("evidence") {
            match self.db.list_evidence(20) {
                Ok(items) if items.is_empty() => self.push("MOTHER: No evidence stored yet."),
                Ok(items) => {
                    self.push("MOTHER: Evidence (recent):");
                    for e in items {
                        self.push(format!(
                            "  [{}] trust {:.2} | {} | {}",
                            e.id, e.trust, e.source, e.excerpt
                        ));
                    }
                }
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // show <concept>
        if let Some(rest) = trimmed.strip_prefix("show ") {
            let name = rest.trim().to_lowercase();
            match self.db.get_concept(&name) {
                Ok(Some(c)) => self.show_concept(&c),
                Ok(None) => self.push(format!("MOTHER: I have no concept named '{}'.", name)),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // claims <concept>
        if let Some(rest) = trimmed.strip_prefix("claims ") {
            let concept = rest.trim().to_lowercase();
            match self.db.list_claims_for(&concept, 20) {
                Ok(items) if items.is_empty() => self.push("MOTHER: No claims stored."),
                Ok(items) => {
                    self.push(format!("MOTHER: Claims for '{}':", concept));
                    for c in items {
                        let evidence_note = if let Some(id) = c.evidence_id {
                            format!(" [evidence {}]", id)
                        } else {
                            "".to_string()
                        };
                        self.push(format!(
                            "  ({:.2}) {}{}",
                            c.confidence, c.claim_text, evidence_note
                        ));
                    }
                }
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // ep <ok|fail|note> <summary>
        if let Some(rest) = trimmed.strip_prefix("ep ") {
            let mut parts = rest.splitn(2, ' ');
            let outcome = parts.next().unwrap_or("").trim().to_lowercase();
            let summary = parts.next().unwrap_or("").trim().to_string();

            let valid = outcome == "ok" || outcome == "fail" || outcome == "note";
            if !valid || summary.is_empty() {
                self.push("MOTHER: Format is: ep ok <what worked> | ep fail <what failed> | ep note <note>");
                return;
            }

            self.pending = Some(PendingAction::Episode {
                outcome: outcome.clone(),
                summary: summary.clone(),
            });
            self.push(format!(
                "MOTHER: PROPOSE EPISODE [{}]: {}",
                outcome, summary
            ));
            self.push("  Approve? [y]/[n]");
            return;
        }

        // learn <concept> is <definition>
        if let Some(rest) = trimmed.strip_prefix("learn ") {
            let parts: Vec<&str> = rest.splitn(2, " is ").collect();
            if parts.len() != 2 {
                self.push("MOTHER: Format is: learn <concept> is <definition>");
                return;
            }

            let name = parts[0].trim().to_lowercase();
            let definition = parts[1].trim().to_string();

            if name.is_empty() || definition.is_empty() {
                self.push("MOTHER: Concept name and definition must be non-empty.");
                return;
            }

            self.pending = Some(PendingAction::Concept {
                name: name.clone(),
                definition: definition.clone(),
                confidence: 0.40,
            });

            self.push("MOTHER: PROPOSAL: STORE CONCEPT");
            self.push(format!("  Concept: {}", name));
            self.push(format!("  Definition: {}", definition));
            self.push("  Confirm? [y]/[n]");
            return;
        }

        // rel <from> <type> <to>
        if let Some(rest) = trimmed.strip_prefix("rel ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() < 3 {
                self.push("MOTHER: Format is: rel <from> <type> <to>");
                self.push("MOTHER: Example: rel jwt used_for authentication");
                return;
            }
            let from = parts[0].trim().to_lowercase();
            let relation_type = parts[1].trim().to_lowercase();
            let to = parts[2..].join(" ").trim().to_lowercase();

            if from.is_empty() || relation_type.is_empty() || to.is_empty() {
                self.push("MOTHER: rel fields must be non-empty.");
                return;
            }

            self.pending = Some(PendingAction::Relation {
                from: from.clone(),
                relation_type: relation_type.clone(),
                to: to.clone(),
            });
            self.push("MOTHER: PROPOSAL: STORE RELATION");
            self.push(format!("  {} --{}--> {}", from, relation_type, to));
            self.push("  Confirm? [y]/[n]");
            return;
        }

        // src <url> :: <excerpt>
        if let Some(rest) = trimmed.strip_prefix("src ") {
            let parts: Vec<&str> = rest.splitn(2, "::").collect();
            if parts.len() != 2 {
                self.push("MOTHER: Format is: src <url> :: <excerpt>");
                return;
            }
            let source = parts[0].trim().to_string();
            let excerpt = parts[1].trim().to_string();
            if source.is_empty() || excerpt.is_empty() {
                self.push("MOTHER: Source and excerpt required.");
                return;
            }
            self.pending = Some(PendingAction::Evidence {
                source: source.clone(),
                excerpt: excerpt.clone(),
                trust: 0.5,
            });
            self.push("MOTHER: PROPOSAL: STORE EVIDENCE");
            self.push(format!("  Source: {}", source));
            self.push(format!("  Excerpt: {}", excerpt));
            self.push("  Confirm? [y]/[n]");
            return;
        }

        // claim <concept> :: <text> :: <evidence_id?>
        if let Some(rest) = trimmed.strip_prefix("claim ") {
            let parts: Vec<&str> = rest.splitn(3, "::").collect();
            if parts.len() < 2 {
                self.push(
                    "MOTHER: Format is: claim <concept> :: <claim text> :: <evidence_id optional>",
                );
                return;
            }
            let concept = parts[0].trim().to_lowercase();
            let claim_text = parts[1].trim().to_string();
            let evidence_id = if parts.len() == 3 {
                let raw = parts[2].trim();
                if raw.is_empty() {
                    None
                } else {
                    raw.parse::<i64>().ok()
                }
            } else {
                None
            };
            if concept.is_empty() || claim_text.is_empty() {
                self.push("MOTHER: Concept and claim text are required.");
                return;
            }
            if let Some(id) = evidence_id {
                match self.db.get_evidence(id) {
                    Ok(Some(e)) => {
                        self.push(format!("MOTHER: Evidence [{}] found: {}", e.id, e.source))
                    }
                    Ok(None) => {
                        self.push(format!(
                            "MOTHER: Evidence id {} not found. Remove or fix.",
                            id
                        ));
                        return;
                    }
                    Err(e) => {
                        self.push(format!("MOTHER: DB error: {}", e));
                        return;
                    }
                }
            }
            self.pending = Some(PendingAction::Claim {
                concept: concept.clone(),
                claim_text: claim_text.clone(),
                evidence_id,
                confidence: 0.5,
            });
            self.push("MOTHER: PROPOSAL: STORE CLAIM");
            self.push(format!("  Concept: {}", concept));
            self.push(format!("  Claim: {}", claim_text));
            if let Some(id) = evidence_id {
                self.push(format!("  Evidence link: {}", id));
            }
            self.push("  Confirm? [y]/[n]");
            return;
        }

        // search <query>
        if let Some(rest) = trimmed.strip_prefix("search ") {
            let query = rest.trim().to_string();
            if query.is_empty() {
                self.push("MOTHER: Provide a query. Example: search rust async traits");
                return;
            }
            self.pending = Some(PendingAction::SearchPlan {
                query: query.clone(),
            });
            self.push("MOTHER: SEARCH PLAN CREATED (permissioned).");
            self.push(format!("  Query: {}", query));
            self.push("  Approve network fetch? [y]/[n]");
            return;
        }

        // keep <n> or keep all
        if let Some(rest) = trimmed.strip_prefix("keep ") {
            if self.search_results.is_empty() {
                self.push("MOTHER: No search results to keep. Run search + approve first.");
                return;
            }
            let selection = rest.trim();
            if selection.eq_ignore_ascii_case("all") {
                self.pending = Some(PendingAction::StoreEvidence {
                    items: self.search_results.clone(),
                });
                self.push(format!(
                    "MOTHER: PROPOSAL: STORE {} evidence items (keep all). Approve? [y]/[n]",
                    self.search_results.len()
                ));
                return;
            }
            if let Ok(idx) = selection.parse::<usize>() {
                if idx == 0 || idx > self.search_results.len() {
                    self.push("MOTHER: keep <n> uses 1-based index within results.");
                    return;
                }
                let item = self.search_results[idx - 1].clone();
                self.pending = Some(PendingAction::StoreEvidence {
                    items: vec![item.clone()],
                });
                self.push(format!(
                    "MOTHER: PROPOSAL: STORE evidence #{} -> {} ... Approve? [y]/[n]",
                    idx, item.source
                ));
                return;
            }
            self.push("MOTHER: keep expects 'keep <n>' or 'keep all'.");
            return;
        }

        // doctor
        if trimmed.eq_ignore_ascii_case("doctor") {
            self.run_doctor();
            return;
        }

        // fallback
        self.push(self.eliza_reflect(trimmed));
    }

    fn show_concept(&mut self, c: &Concept) {
        self.push("MOTHER: CONCEPT RECORD");
        self.push(format!("  Name: {}", c.name));
        self.push(format!("  Definition: {}", c.definition));
        self.push(format!("  Confidence: {:.2}", c.confidence));
        self.push(format!("  Created: {}", c.created_at));

        if let Ok(claims) = self.db.list_claims_for(&c.name, 10) {
            if !claims.is_empty() {
                self.push("  Claims:");
                for cl in claims {
                    let evidence_note = if let Some(id) = cl.evidence_id {
                        format!(" [evidence {}]", id)
                    } else {
                        "".to_string()
                    };
                    self.push(format!(
                        "    ({:.2}) {}{}",
                        cl.confidence, cl.claim_text, evidence_note
                    ));
                }
            }
        }
    }

    fn confirm_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            match p {
                PendingAction::Concept {
                    name,
                    definition,
                    confidence,
                } => match self.db.upsert_concept(&name, &definition, confidence) {
                    Ok(()) => {
                        self.push("MOTHER: COMMITTED CONCEPT.");
                        self.push(format!("  Stored '{}'.", name));
                    }
                    Err(e) => self.push(format!("MOTHER: DB error committing concept: {}", e)),
                },
                PendingAction::Relation {
                    from,
                    relation_type,
                    to,
                } => match self.db.upsert_relation(&from, &relation_type, &to) {
                    Ok(()) => self.push(format!(
                        "MOTHER: COMMITTED RELATION {} --{}--> {}",
                        from, relation_type, to
                    )),
                    Err(e) => self.push(format!("MOTHER: DB error committing relation: {}", e)),
                },
                PendingAction::Episode { outcome, summary } => {
                    match self.db.add_episode(&outcome, &summary) {
                        Ok(()) => self.push(format!(
                            "MOTHER: RECORDED EPISODE [{}] {}",
                            outcome, summary
                        )),
                        Err(e) => self.push(format!("MOTHER: DB error committing episode: {}", e)),
                    }
                }
                PendingAction::Evidence {
                    source,
                    excerpt,
                    trust,
                } => match self.db.add_evidence(&source, &excerpt, trust) {
                    Ok(id) => self.push(format!("MOTHER: STORED EVIDENCE [{}]", id)),
                    Err(e) => self.push(format!("MOTHER: DB error committing evidence: {}", e)),
                },
                PendingAction::Claim {
                    concept,
                    claim_text,
                    evidence_id,
                    confidence,
                } => {
                    match self
                        .db
                        .add_claim(&concept, &claim_text, evidence_id, confidence)
                    {
                        Ok(id) => {
                            self.push(format!("MOTHER: STORED CLAIM [{}] for '{}'", id, concept))
                        }
                        Err(e) => self.push(format!("MOTHER: DB error committing claim: {}", e)),
                    }
                }
                PendingAction::SearchPlan { query } => {
                    self.run_search(&query);
                }
                PendingAction::StoreEvidence { items } => {
                    let mut stored = 0usize;
                    for item in items {
                        if self
                            .db
                            .add_evidence(&item.source, &item.excerpt, 0.5)
                            .is_ok()
                        {
                            stored += 1;
                        }
                    }
                    self.push(format!("MOTHER: STORED {} evidence items.", stored));
                }
            }
        } else {
            self.push("MOTHER: No pending proposal.");
        }
    }

    fn reject_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            let label = match p {
                PendingAction::Concept { .. } => "Concept",
                PendingAction::Relation { .. } => "Relation",
                PendingAction::Episode { .. } => "Episode",
                PendingAction::Evidence { .. } => "Evidence",
                PendingAction::Claim { .. } => "Claim",
                PendingAction::SearchPlan { .. } => "Search",
                PendingAction::StoreEvidence { .. } => "Store evidence",
            };
            self.push(format!("MOTHER: {} proposal rejected.", label));
        } else {
            self.push("MOTHER: No pending proposal.");
        }
    }

    fn run_doctor(&mut self) {
        let lynx = Command::new("which").arg("lynx").output();
        let curl = Command::new("which").arg("curl").output();
        let lynx_status = if let Ok(out) = lynx {
            if out.status.success() {
                "lynx: available"
            } else {
                "lynx: missing"
            }
        } else {
            "lynx: check failed"
        };
        let curl_status = if let Ok(out) = curl {
            if out.status.success() {
                "curl: available"
            } else {
                "curl: missing"
            }
        } else {
            "curl: check failed"
        };
        self.push("MOTHER: TOOLING DOCTOR");
        self.push(format!("  {}", lynx_status));
        self.push(format!("  {}", curl_status));
        self.push("  Search remains permissioned; approval required.");
    }

    fn run_search(&mut self, query: &str) {
        self.search_results.clear();
        self.push(format!(
            "MOTHER: Running permissioned search for '{}'",
            query
        ));
        let lynx_ok = Command::new("which")
            .arg("lynx")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let command_used: String;
        let output = if lynx_ok {
            command_used = "lynx -dump".into();
            Command::new("lynx").args(["-dump", query]).output()
        } else {
            command_used = "curl -L".into();
            Command::new("curl").args(["-L", query]).output()
        };

        match output {
            Ok(result) if result.status.success() => {
                let text = String::from_utf8_lossy(&result.stdout);
                self.search_results = extract_candidates(&text);
                self.push(format!(
                    "MOTHER: Search completed via {} ({} candidates)",
                    command_used,
                    self.search_results.len()
                ));
                let preview: Vec<_> = self.search_results.iter().take(5).cloned().collect();
                for (i, item) in preview.iter().enumerate() {
                    self.push(format!("  [{}] {} :: {}", i + 1, item.source, item.excerpt));
                }
                if self.search_results.is_empty() {
                    self.push("  No evidence candidates parsed.");
                } else {
                    self.push("  Use keep <n> or keep all to propose storage (approval required).");
                }
            }
            Ok(result) => {
                self.push(format!(
                    "MOTHER: Search command failed ({}). stderr len {}",
                    command_used,
                    result.stderr.len()
                ));
            }
            Err(e) => {
                self.push(format!(
                    "MOTHER: Search failed to run {}: {}",
                    command_used, e
                ));
            }
        }
    }
}

impl Module for Dialog {
    fn render(&mut self, f: &mut Frame, status: &str) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(f.area());

        let banner =
            Paragraph::new(status).block(Block::default().borders(Borders::ALL).title("STATUS"));

        let text = self.history.join("\n");
        let dialog =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("DIALOG"));

        let input = Paragraph::new(self.input.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("INPUT (Enter to send, y/n to confirm pending)"),
        );

        f.render_widget(banner, layout[0]);
        f.render_widget(dialog, layout[1]);
        f.render_widget(input, layout[2]);
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') if self.pending.is_some() => self.confirm_pending(),
            KeyCode::Char('n') if self.pending.is_some() => self.reject_pending(),
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                let line = self.input.clone();
                self.push(format!("YOU: {}", line));
                self.input.clear();
                self.handle_command(&line);
            }
            KeyCode::Esc => {
                if self.pending.is_some() {
                    self.reject_pending();
                } else {
                    self.push("MOTHER: Nothing to cancel.");
                }
            }
            _ => {}
        }
    }
}

fn extract_candidates(text: &str) -> Vec<EvidenceCandidate> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let excerpt = trimmed
                .split_whitespace()
                .take(50)
                .collect::<Vec<&str>>()
                .join(" ");
            out.push(EvidenceCandidate {
                source: trimmed.to_string(),
                excerpt,
            });
        } else if trimmed.len() > 50 && out.len() < 5 {
            // capture contextual excerpt even without URL for fallback
            out.push(EvidenceCandidate {
                source: "search-output".to_string(),
                excerpt: trimmed.to_string(),
            });
        }
        if out.len() >= 20 {
            break;
        }
    }
    out
}
