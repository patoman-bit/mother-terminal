use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use super::Module;
use crate::db::{Concept, Database};
use crate::search;

#[derive(Clone, Debug)]
struct Proposal {
    name: String,
    definition: String,
    confidence: f64,
}

pub struct Dialog {
    input: String,
    history: Vec<String>,
    db: Database,
    pending: Option<Proposal>,
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
                "  search <url>".into(),
                "MOTHER: If a proposal appears: press [y] to confirm, [n] to reject.".into(),
            ],
            db,
            pending: None,
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
                Ok(()) => self.push(format!(
                    "MOTHER: EPISODE RECORDED [{}] {}",
                    outcome, summary
                )),
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

            self.pending = Some(Proposal {
                name: name.clone(),
                definition: definition.clone(),
                confidence: 0.40,
            });

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
                Ok(()) => self.push(format!(
                    "MOTHER: Linked {} --{}--> {}",
                    from, relation_type, to
                )),
                Err(e) => self.push(format!("MOTHER: DB error: {}", e)),
            }
            return;
        }

        if trimmed.eq_ignore_ascii_case("doctor") {
            self.doctor();
            return;
        }

        if let Some(rest) = trimmed.strip_prefix("search ") {
            let url = rest.trim();
            if url.is_empty() {
                self.push("MOTHER: Format is: search <url>");
                return;
            }
            self.execute_search(url);
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
            match self.db.upsert_concept(&p.name, &p.definition, p.confidence) {
                Ok(()) => {
                    self.push("MOTHER: COMMITTED.");
                    self.push(format!("  Stored concept '{}'.", p.name));
                }
                Err(e) => self.push(format!("MOTHER: DB error committing proposal: {}", e)),
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

    fn doctor(&mut self) {
        self.push("MOTHER: SYSTEM DIAGNOSTIC");

        match self.db.list_concepts(1) {
            Ok(_) => self.push("  DB: READY"),
            Err(e) => self.push(format!("  DB: ERROR ({})", e)),
        }

        let tools = search::probe_tools();
        self.push(format!("  SEARCH: lynx {}", status_word(tools.lynx)));
        self.push(format!("  SEARCH: curl {}", status_word(tools.curl)));

        if tools.ready() {
            self.push("  SEARCH: READY FOR APPROVED QUERIES");
        } else {
            self.push("  SEARCH: lynx is required for execution");
        }
    }

    fn execute_search(&mut self, url: &str) {
        self.push(format!("MOTHER: APPROVED SEARCH -> {}", url));
        match search::run_search(url) {
            Ok(result) => {
                if result.candidates.is_empty() {
                    self.push("MOTHER: No candidates detected in output.");
                } else {
                    self.push("MOTHER: CANDIDATE LINKS/TITLES:");
                    for (idx, line) in result.candidates.iter().take(12).enumerate() {
                        self.push(format!("  {}. {}", idx + 1, line));
                    }
                    if result.candidates.len() > 12 {
                        self.push("  ...");
                    }
                }

                self.push("MOTHER: RAW OUTPUT (first 20 lines):");
                for line in result.raw_text.lines().take(20) {
                    self.push(format!("  {}", line));
                }
                if result.raw_text.lines().count() > 20 {
                    self.push("  ...");
                }
            }
            Err(err) => self.push(format!("MOTHER: SEARCH ERROR: {}", err)),
        }
    }
}

impl Module for Dialog {
    fn render(&mut self, f: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(f.area());

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

fn status_word(ok: bool) -> &'static str {
    if ok { "OK" } else { "MISSING" }
}
