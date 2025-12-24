use ratatui::{
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Direction, Constraint},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};

use super::Module;
use crate::db::{Database, Concept};

#[derive(Clone, Debug)]
struct Proposal {
    name: String,
    definition: String,
    confidence: f64,
}

#[derive(Clone, Debug)]
struct SearchPlan {
    query: String,
}

#[derive(Clone, Debug)]
struct EvidenceCandidate {
    title: String,
    snippet: String,
}

#[derive(Clone, Debug)]
struct StoredEvidence {
    id: usize,
    title: String,
    snippet: String,
}

#[derive(Clone, Debug)]
struct ClaimProposal {
    concept: String,
    statement: String,
    evidence_id: usize,
}

#[derive(Clone, Debug)]
struct ClaimRecord {
    concept: String,
    statement: String,
    evidence: StoredEvidence,
}

#[derive(Clone, Debug)]
enum EvidenceSelection {
    Single(usize),
    All,
}

#[derive(Clone, Debug)]
struct EvidenceStoreIntent {
    selection: EvidenceSelection,
}

#[derive(Clone, Debug)]
struct DoctorPlan;

#[derive(Clone, Debug)]
enum PendingAction {
    ConceptProposal(Proposal),
    SearchPlan(SearchPlan),
    EvidenceStore(EvidenceStoreIntent),
    ClaimProposal(ClaimProposal),
    Doctor(DoctorPlan),
}

pub struct Dialog {
    input: String,
    history: Vec<String>,
    db: Database,
    pending: Option<PendingAction>,
    last_search_results: Vec<EvidenceCandidate>,
    kept_evidence: Vec<StoredEvidence>,
    next_evidence_id: usize,
    claims: Vec<ClaimRecord>,
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
                "  ep ok <what worked>".into(),
                "  ep fail <what failed>".into(),
                "  ep note <note>".into(),
                "  episodes".into(),
                "  show <concept>".into(),
                "  list".into(),
                "  doctor".into(),
                "  search <query>".into(),
                "  keep <n|all>".into(),
                "  claim <concept> :: <statement> :: <evidence_id>".into(),
                "MOTHER: If a proposal appears: press [y] to confirm, [n] to reject.".into(),
            ],
            db,
            pending: None,
            last_search_results: Vec::new(),
            kept_evidence: Vec::new(),
            next_evidence_id: 1,
            claims: Vec::new(),
        }
    }

    fn push(&mut self, line: impl Into<String>) {
        self.history.push(line.into());
        if self.history.len() > 240 {
            self.history.drain(0..70);
        }
    }

    fn eliza_reflect(&self, text: &str) -> String {
        format!("MOTHER: Why do you say '{}'?",&text)
    }

    fn handle_doctor(&mut self) {
        if self.pending.is_some() {
            self.push("MOTHER: A pending action awaits confirmation. Approve or reject it first.");
            return;
        }

        self.pending = Some(PendingAction::Doctor(DoctorPlan));
        self.push("MOTHER: PLAN: run offline diagnostics (no network).");
        self.push("  Steps: (1) Inspect local knowledge counts. (2) Surface possible follow-up commands.");
        self.push("MOTHER: Proceed with doctor? [y]es / [n]o");
    }

    fn handle_command(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        if self.pending.is_some() {
            self.push("MOTHER: A pending action awaits [y]/[n]. Please confirm or reject first.");
            return;
        }

        // episodes
        if trimmed.eq_ignore_ascii_case("episodes") {
            match self.db.list_episodes(20) {
                Ok(items) if items.is_empty() => self.push("MOTHER: No episodes stored yet."),
                Ok(items) => {
                    self.push("MOTHER: Recent episodes:");
                    for e in items {
                        self.push(format!("  - [{}] {}  {}", e.outcome, e.captured_at, e.summary));
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

            match self.db.add_episode(&outcome, &summary) {
                Ok(()) => self.push(format!("MOTHER: EPISODE RECORDED [{}] {}", outcome, summary)),
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

        // doctor
        if trimmed.eq_ignore_ascii_case("doctor") {
            self.handle_doctor();
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

            self.pending = Some(PendingAction::ConceptProposal(Proposal {
                name: name.clone(),
                definition: definition.clone(),
                confidence: 0.40,
            }));

            self.push("MOTHER: PROPOSAL CREATED.");
            self.push(format!("  Concept: {}", name));
            self.push(format!("  Definition: {}", definition));
            self.push("MOTHER: Confirm? [y]es / [n]o");
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

            match self.db.upsert_relation(&from, &relation_type, &to) {
                Ok(()) => self.push(format!("MOTHER: Linked {} --{}--> {}", from, relation_type, to)),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        // search <query>
        if let Some(rest) = trimmed.strip_prefix("search ") {
            let query = rest.trim();
            if query.is_empty() {
                self.push("MOTHER: Provide a query. Format: search <query>");
                return;
            }

            self.pending = Some(PendingAction::SearchPlan(SearchPlan { query: query.to_string() }));
            self.push("MOTHER: PLAN: prepare search without contacting external sources.");
            self.push(format!("  Query: {}", query));
            self.push("  Steps: (1) Await approval. (2) Execute offline search strategy. (3) Present numbered candidates.".
                to_string());
            self.push("MOTHER: Proceed with search? [y]es / [n]o");
            return;
        }

        // keep <n|all>
        if let Some(rest) = trimmed.strip_prefix("keep ") {
            let arg = rest.trim().to_lowercase();
            if self.last_search_results.is_empty() {
                self.push("MOTHER: No search results available to keep. Run 'search <query>' first.");
                return;
            }

            let selection = if arg == "all" {
                EvidenceSelection::All
            } else if let Ok(idx) = arg.parse::<usize>() {
                if idx == 0 {
                    self.push("MOTHER: Evidence numbers start at 1.");
                    return;
                }
                EvidenceSelection::Single(idx)
            } else {
                self.push("MOTHER: Format is: keep <n|all> (example: keep 2)");
                return;
            };

            self.pending = Some(PendingAction::EvidenceStore(EvidenceStoreIntent { selection: selection.clone() }));
            match selection {
                EvidenceSelection::All => {
                    self.push("MOTHER: Proposal: store all shown evidence candidates.".to_string());
                }
                EvidenceSelection::Single(idx) => {
                    if let Some(c) = self.last_search_results.get(idx.saturating_sub(1)) {
                        self.push(format!("MOTHER: Proposal: store evidence #{}: {}", idx, c.title));
                    } else {
                        self.push(format!("MOTHER: Evidence #{} is not available. Use a number from the last search.", idx));
                        self.pending = None;
                        return;
                    }
                }
            }
            self.push("MOTHER: Confirm? [y]es / [n]o");
            return;
        }

        // claim <concept> :: <statement> :: <evidence_id>
        if let Some(rest) = trimmed.strip_prefix("claim ") {
            let parts: Vec<&str> = rest.split("::").map(|p| p.trim()).collect();
            if parts.len() != 3 {
                self.push("MOTHER: Format is: claim <concept> :: <statement> :: <evidence_id>");
                return;
            }

            let concept = parts[0].to_lowercase();
            let statement = parts[1].to_string();
            let evidence_id: usize = match parts[2].parse() {
                Ok(id) if id > 0 => id,
                _ => {
                    self.push("MOTHER: Evidence id must be a positive number.");
                    return;
                }
            };

            let evidence = self.kept_evidence.iter().find(|e| e.id == evidence_id).cloned();
            let Some(evidence) = evidence else {
                self.push(format!("MOTHER: No stored evidence with id {}. Use 'keep <n|all>' first.", evidence_id));
                return;
            };

            if concept.is_empty() || statement.is_empty() {
                self.push("MOTHER: Concept and statement must be non-empty.");
                return;
            }

            self.pending = Some(PendingAction::ClaimProposal(ClaimProposal {
                concept: concept.clone(),
                statement: statement.clone(),
                evidence_id,
            }));

            self.push("MOTHER: CLAIM PROPOSAL.".to_string());
            self.push(format!("  Concept: {}", concept));
            self.push(format!("  Statement: {}", statement));
            self.push(format!("  Evidence: #{} {}", evidence.id, evidence.title));
            self.push("MOTHER: Confirm? [y]es / [n]o");
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
    }

    fn confirm_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            match p {
                PendingAction::ConceptProposal(p) => {
                    match self.db.upsert_concept(&p.name, &p.definition, p.confidence) {
                        Ok(()) => {
                            self.push("MOTHER: COMMITTED.");
                            self.push(format!("  Stored concept '{}'.", p.name));
                        }
                        Err(e) => self.push(format!("MOTHER: DB error committing proposal: {}", e)),
                    }
                }
                PendingAction::SearchPlan(plan) => {
                    self.run_search(plan);
                }
                PendingAction::EvidenceStore(intent) => {
                    self.store_evidence(intent);
                }
                PendingAction::ClaimProposal(proposal) => {
                    self.record_claim(proposal);
                }
                PendingAction::Doctor(_) => {
                    self.run_doctor();
                }
            }
        } else {
            self.push("MOTHER: No pending proposal.");
        }
    }

    fn reject_pending(&mut self) {
        if self.pending.take().is_some() {
            self.push("MOTHER: Proposal rejected.");
        } else {
            self.push("MOTHER: No pending proposal.");
        }
    }

    fn run_doctor(&mut self) {
        match self.db.list_concepts(5) {
            Ok(concepts) => {
                self.push("MOTHER: DOCTOR REPORT (local).");
                self.push(format!("  Concepts stored: {}", concepts.len()));
                if concepts.is_empty() {
                    self.push("  Tip: add knowledge using 'learn <concept> is <definition>'");
                } else {
                    self.push("  Tip: explore relations with 'rel <from> <type> <to>'");
                }
                if self.last_search_results.is_empty() {
                    self.push("  Tip: try 'search <query>' to gather candidates (requires approval).");
                } else {
                    self.push("  Tip: consider 'keep <n|all>' to persist evidence candidates.");
                }
            }
            Err(e) => {
                self.push(format!("MOTHER: Doctor encountered a DB error: {}", e));
            }
        }
    }

    fn run_search(&mut self, plan: SearchPlan) {
        self.push("MOTHER: EXECUTING APPROVED SEARCH PLAN (offline).");
        let query = plan.query;
        self.last_search_results = vec![
            EvidenceCandidate {
                title: format!("{} overview", query),
                snippet: "High-level summary derived from local heuristics.".to_string(),
            },
            EvidenceCandidate {
                title: format!("{} key sources", query),
                snippet: "List of potential references to investigate manually.".to_string(),
            },
            EvidenceCandidate {
                title: format!("{} risks", query),
                snippet: "Possible pitfalls to validate once external access is allowed.".to_string(),
            },
        ];

        self.push("MOTHER: SEARCH COMPLETE. Candidates ready (no external calls were made).");
        for i in 0..self.last_search_results.len() {
            let formatted = {
                let c = &self.last_search_results[i];
                format!("  [{}] {} — {}", i + 1, c.title, c.snippet)
            };
            self.push(formatted);
        }
        self.push("MOTHER: Use 'keep <n|all>' to store evidence after review.");
    }

    fn store_evidence(&mut self, intent: EvidenceStoreIntent) {
        match intent.selection {
            EvidenceSelection::All => {
                if self.last_search_results.is_empty() {
                    self.push("MOTHER: No search results to store.");
                    return;
                }
                let candidates: Vec<EvidenceCandidate> = self.last_search_results.clone();
                for c in candidates {
                    let id = self.next_evidence_id;
                    self.next_evidence_id += 1;
                    self.kept_evidence.push(StoredEvidence {
                        id,
                        title: c.title.clone(),
                        snippet: c.snippet.clone(),
                    });
                    self.push(format!("MOTHER: STORED evidence #{}: {}", id, c.title));
                }
            }
            EvidenceSelection::Single(idx) => {
                let Some(c) = self.last_search_results.get(idx.saturating_sub(1)) else {
                    self.push(format!("MOTHER: Evidence #{} not found in last search.", idx));
                    return;
                };
                let id = self.next_evidence_id;
                self.next_evidence_id += 1;
                self.kept_evidence.push(StoredEvidence {
                    id,
                    title: c.title.clone(),
                    snippet: c.snippet.clone(),
                });
                self.push(format!("MOTHER: STORED evidence #{}: {}", id, c.title));
            }
        }
    }

    fn record_claim(&mut self, proposal: ClaimProposal) {
        let Some(evidence) = self.kept_evidence.iter().find(|e| e.id == proposal.evidence_id).cloned() else {
            self.push(format!("MOTHER: Evidence #{} missing. Cannot record claim.", proposal.evidence_id));
            return;
        };

        self.claims.push(ClaimRecord {
            concept: proposal.concept.clone(),
            statement: proposal.statement.clone(),
            evidence: evidence.clone(),
        });

        self.push("MOTHER: CLAIM RECORDED.");
        self.push(format!("  Concept: {}", proposal.concept));
        self.push(format!("  Statement: {}", proposal.statement));
        self.push(format!("  Evidence linked: #{} {}", evidence.id, evidence.title));
    }
}

impl Module for Dialog {
    fn render(&mut self, f: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(f.area());

        let text = self.history.join("\n");
        let dialog = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("DIALOG"));

        let input = Paragraph::new(self.input.as_str())
            .block(Block::default().borders(Borders::ALL).title("INPUT"));

        f.render_widget(dialog, layout[0]);
        f.render_widget(input, layout[1]);
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') if self.pending.is_some() => self.confirm_pending(),
            KeyCode::Char('n') if self.pending.is_some() => self.reject_pending(),
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Backspace => { self.input.pop(); }
            KeyCode::Enter => {
                let line = self.input.clone();
                self.push(format!("YOU: {}", line));
                self.input.clear();
                self.handle_command(&line);
            }
            _ => {}
        }
    }
}
