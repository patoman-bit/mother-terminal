use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
};

use super::Module;
use crate::db::{Concept, Database};

#[derive(Clone, Debug)]
struct ConceptProposal {
    name: String,
    definition: String,
    confidence: f64,
}

#[derive(Clone, Debug)]
struct SearchPlan {
    query: String,
    provider: SearchProvider,
}

#[derive(Clone, Debug, Copy)]
enum SearchProvider {
    Lynx,
    Curl,
}

#[derive(Clone, Debug)]
struct EvidenceCandidate {
    content: String,
    source: String,
}

#[derive(Clone, Debug)]
struct EvidenceProposal {
    concept: String,
    items: Vec<EvidenceCandidate>,
}

#[derive(Clone, Debug)]
struct ClaimProposal {
    concept: String,
    statement: String,
    evidence_id: Option<i64>,
    confidence: f64,
}

#[derive(Clone, Debug)]
enum PendingAction {
    Concept(ConceptProposal),
    Search(SearchPlan),
    Evidence(EvidenceProposal),
    Claim(ClaimProposal),
}

#[derive(Clone, Debug, Default)]
struct ToolStatus {
    lynx: bool,
    curl: bool,
    checked: bool,
}

pub struct Dialog {
    input: String,
    history: Vec<String>,
    db: Database,
    pending: Option<PendingAction>,
    tools: ToolStatus,
    search_candidates: Vec<EvidenceCandidate>,
    search_context: Option<String>,
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
                "  episodes | list | show <concept>".into(),
                "  doctor (tool check)".into(),
                "  search <query> (plan only, confirmation required)".into(),
                "  keep <n>|all (propose storing search result as evidence)".into(),
                "  claim <concept> [evidence <id>] <statement>".into(),
                "MOTHER: If a proposal appears: press [y] to confirm, [n] to reject.".into(),
            ],
            db,
            pending: None,
            tools: ToolStatus::default(),
            search_candidates: Vec::new(),
            search_context: None,
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

        // episodes
        if trimmed.eq_ignore_ascii_case("episodes") {
            self.handle_episodes();
            return;
        }

        // doctor: probe tools
        if trimmed.eq_ignore_ascii_case("doctor") {
            self.check_tools();
            return;
        }

        // ep <ok|fail|note> <summary>
        if let Some(rest) = trimmed.strip_prefix("ep ") {
            self.handle_episode_command(rest);
            return;
        }

        // list concepts
        if trimmed.eq_ignore_ascii_case("list") {
            self.list_concepts();
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
            self.propose_concept(rest);
            return;
        }

        // rel <from> <type> <to>
        if let Some(rest) = trimmed.strip_prefix("rel ") {
            self.handle_relation(rest);
            return;
        }

        // search <query>
        if let Some(rest) = trimmed.strip_prefix("search ") {
            self.plan_search(rest);
            return;
        }

        // keep <n>|all
        if let Some(rest) = trimmed.strip_prefix("keep ") {
            self.plan_keep(rest);
            return;
        }

        // claim ...
        if let Some(rest) = trimmed.strip_prefix("claim ") {
            self.plan_claim(rest);
            return;
        }

        // fallback
        self.push(self.eliza_reflect(trimmed));
    }

    fn handle_episodes(&mut self) {
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
    }

    fn handle_episode_command(&mut self, rest: &str) {
        let mut parts = rest.splitn(2, ' ');
        let outcome = parts.next().unwrap_or("").trim().to_lowercase();
        let summary = parts.next().unwrap_or("").trim().to_string();

        let valid = outcome == "ok" || outcome == "fail" || outcome == "note";
        if !valid || summary.is_empty() {
            self.push(
                "MOTHER: Format is: ep ok <what worked> | ep fail <what failed> | ep note <note>",
            );
            return;
        }

        match self.db.add_episode(&outcome, &summary) {
            Ok(()) => self.push(format!(
                "MOTHER: EPISODE RECORDED [{}] {}",
                outcome, summary
            )),
            Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
        }
    }

    fn list_concepts(&mut self) {
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
    }

    fn propose_concept(&mut self, rest: &str) {
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

        self.pending = Some(PendingAction::Concept(ConceptProposal {
            name: name.clone(),
            definition: definition.clone(),
            confidence: 0.40,
        }));

        self.push("MOTHER: PROPOSAL CREATED.");
        self.push(format!("  Concept: {}", name));
        self.push(format!("  Definition: {}", definition));
        self.push("MOTHER: Confirm? [y]es / [n]o");
    }

    fn handle_relation(&mut self, rest: &str) {
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
            Ok(()) => self.push(format!(
                "MOTHER: Linked {} --{}--> {}",
                from, relation_type, to
            )),
            Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
        }
    }

    fn plan_search(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.push("MOTHER: search requires a query.");
            return;
        }

        if !self.tools.checked {
            self.check_tools();
        }

        let provider = self.preferred_provider();
        if provider.is_none() {
            self.push(
                "MOTHER: No search provider available. Run 'doctor' to install lynx or curl.",
            );
            return;
        }

        let plan = SearchPlan {
            query: query.to_string(),
            provider: provider.unwrap(),
        };
        self.pending = Some(PendingAction::Search(plan.clone()));
        self.push(format!(
            "MOTHER: SEARCH PLAN: provider={} query='{}'. Confirm? [y]/[n]",
            plan.provider.as_str(),
            plan.query
        ));
    }

    fn plan_keep(&mut self, selection: &str) {
        if self.search_candidates.is_empty() {
            self.push("MOTHER: No search candidates to keep. Run a search first.");
            return;
        }

        let selection = selection.trim().to_lowercase();
        let chosen: Vec<EvidenceCandidate> = if selection == "all" {
            self.search_candidates.clone()
        } else if let Ok(idx) = selection.parse::<usize>() {
            if idx == 0 || idx > self.search_candidates.len() {
                self.push(format!(
                    "MOTHER: keep index out of range (1-{}).",
                    self.search_candidates.len()
                ));
                return;
            }
            vec![self.search_candidates[idx - 1].clone()]
        } else {
            self.push("MOTHER: keep requires 'all' or an index like keep 1");
            return;
        };

        let concept = self
            .search_context
            .clone()
            .unwrap_or_else(|| "search".into());
        self.pending = Some(PendingAction::Evidence(EvidenceProposal {
            concept,
            items: chosen,
        }));
        self.push("MOTHER: Evidence proposal created from search candidates. Confirm? [y]/[n]");
    }

    fn plan_claim(&mut self, rest: &str) {
        let mut parts = rest.split_whitespace();
        let concept = parts.next().unwrap_or("").trim().to_lowercase();
        if concept.is_empty() {
            self.push("MOTHER: claim requires a concept name.");
            return;
        }
        let remaining: String = parts.collect::<Vec<&str>>().join(" ");
        if remaining.is_empty() {
            self.push("MOTHER: claim requires a statement.");
            return;
        }

        let mut evidence_id = None;
        let statement: String;
        if let Some(rest_statement) = remaining.strip_prefix("evidence ") {
            let mut parts = rest_statement.splitn(2, ' ');
            let id_str = parts.next().unwrap_or("");
            let parsed = id_str.parse::<i64>().ok();
            let text_part = parts.next().unwrap_or("").trim().to_string();
            if parsed.is_none() || text_part.is_empty() {
                self.push(
                    "MOTHER: claim evidence form is: claim <concept> evidence <id> <statement>",
                );
                return;
            }
            evidence_id = parsed;
            statement = text_part;
        } else {
            statement = remaining.trim().to_string();
        }

        let proposal = ClaimProposal {
            concept: concept.clone(),
            statement: statement.clone(),
            evidence_id,
            confidence: 0.40,
        };
        self.pending = Some(PendingAction::Claim(proposal));
        self.push("MOTHER: CLAIM PROPOSAL CREATED. Confirm? [y]/[n]");
        self.push(format!("  Concept: {}", concept));
        if let Some(id) = evidence_id {
            self.push(format!("  Evidence: {}", id));
        }
        self.push(format!("  Statement: {}", statement));
    }

    fn show_concept(&mut self, c: &Concept) {
        self.push("MOTHER: CONCEPT RECORD");
        self.push(format!("  Name: {}", c.name));
        self.push(format!("  Definition: {}", c.definition));
        self.push(format!("  Confidence: {:.2}", c.confidence));
        self.push(format!("  Created: {}", c.created_at));

        if let Ok(evidence) = self.db.list_evidence_for(&c.name, 5) {
            if !evidence.is_empty() {
                self.push("  Evidence (recent):");
                for ev in evidence {
                    self.push(format!("    [{}] {} :: {}", ev.id, ev.source, ev.content));
                }
            }
        }

        if let Ok(claims) = self.db.list_claims_for(&c.name, 5) {
            if !claims.is_empty() {
                self.push("  Claims (recent):");
                for cl in claims {
                    let ev = cl
                        .evidence_id
                        .map(|id| format!(" evidence={}", id))
                        .unwrap_or_default();
                    self.push(format!("    [{}]{ev} {}", cl.id, cl.statement));
                }
            }
        }
    }

    fn confirm_pending(&mut self) {
        let Some(action) = self.pending.take() else {
            self.push("MOTHER: No pending proposal.");
            return;
        };

        match action {
            PendingAction::Concept(p) => {
                match self.db.upsert_concept(&p.name, &p.definition, p.confidence) {
                    Ok(()) => {
                        self.push("MOTHER: CONCEPT COMMITTED.");
                        self.push(format!("  Stored '{}'.", p.name));
                    }
                    Err(e) => self.push(format!("MOTHER: DB error committing proposal: {}", e)),
                }
            }
            PendingAction::Search(plan) => {
                self.run_search(plan);
            }
            PendingAction::Evidence(ev) => {
                for item in ev.items.iter() {
                    match self
                        .db
                        .add_evidence(&ev.concept, &item.content, &item.source)
                    {
                        Ok(id) => self.push(format!(
                            "MOTHER: Evidence [{}] stored for {}",
                            id, ev.concept
                        )),
                        Err(e) => self.push(format!("MOTHER: DB error storing evidence: {}", e)),
                    }
                }
            }
            PendingAction::Claim(c) => {
                match self
                    .db
                    .add_claim(&c.concept, &c.statement, c.evidence_id, c.confidence)
                {
                    Ok(id) => self.push(format!("MOTHER: Claim [{}] stored for {}", id, c.concept)),
                    Err(e) => self.push(format!("MOTHER: DB error storing claim: {}", e)),
                }
            }
        }
    }

    fn reject_pending(&mut self) {
        if let Some(action) = self.pending.take() {
            let label = match action {
                PendingAction::Concept(_) => "concept proposal",
                PendingAction::Search(_) => "search plan",
                PendingAction::Evidence(_) => "evidence proposal",
                PendingAction::Claim(_) => "claim proposal",
            };
            self.push(format!("MOTHER: {} rejected.", label));
        } else {
            self.push("MOTHER: No pending proposal.");
        }
    }

    fn check_tools(&mut self) {
        self.tools.lynx = self.tool_available("lynx");
        self.tools.curl = self.tool_available("curl");
        self.tools.checked = true;

        self.push("MOTHER: TOOL CHECK");
        self.push(format!(
            "  lynx: {}",
            if self.tools.lynx {
                "available"
            } else {
                "missing"
            }
        ));
        self.push(format!(
            "  curl: {}",
            if self.tools.curl {
                "available"
            } else {
                "missing"
            }
        ));
    }

    fn tool_available(&self, tool: &str) -> bool {
        Command::new("which")
            .arg(tool)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn preferred_provider(&self) -> Option<SearchProvider> {
        if self.tools.lynx {
            Some(SearchProvider::Lynx)
        } else if self.tools.curl {
            Some(SearchProvider::Curl)
        } else {
            None
        }
    }

    fn run_search(&mut self, plan: SearchPlan) {
        self.push(format!(
            "MOTHER: Running search via {} (operator approved).",
            plan.provider.as_str()
        ));

        let query = plan.query.replace(' ', "+");
        let url = format!("https://duckduckgo.com/html/?q={}", query);
        let output = match plan.provider {
            SearchProvider::Lynx => Command::new("lynx").args(["-dump", &url]).output(),
            SearchProvider::Curl => Command::new("curl").args(["-sL", &url]).output(),
        };

        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut candidates = Vec::new();
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    candidates.push(EvidenceCandidate {
                        content: trimmed.to_string(),
                        source: plan.provider.as_str().to_string(),
                    });
                    if candidates.len() >= 8 {
                        break;
                    }
                }

                if candidates.is_empty() {
                    self.push("MOTHER: No candidate lines returned.");
                } else {
                    self.search_candidates = candidates.clone();
                    self.search_context = Some(plan.query.clone());
                    self.push("MOTHER: SEARCH CANDIDATES (use keep <n>|all to propose evidence):");
                    for (i, cand) in candidates.iter().enumerate() {
                        self.push(format!("  [{}] {}", i + 1, cand.content));
                    }
                }
            }
            Ok(out) => {
                self.push(format!(
                    "MOTHER: Search provider failed (status {}).",
                    out.status
                ));
            }
            Err(e) => self.push(format!("MOTHER: Search provider error: {}", e)),
        }
    }
}

impl SearchProvider {
    fn as_str(&self) -> &'static str {
        match self {
            SearchProvider::Lynx => "lynx",
            SearchProvider::Curl => "curl",
        }
    }
}

impl Module for Dialog {
    fn render(&mut self, f: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(area);

        let text = self.history.join("\n");
        let dialog =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("DIALOG"));

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
            KeyCode::Backspace => {
                self.input.pop();
            }
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
