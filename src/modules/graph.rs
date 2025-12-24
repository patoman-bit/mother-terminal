use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::Module;
use crate::db::{Database, Relation};
use rusqlite::Result;

pub struct Graph {
    db: Database,
    concepts: Vec<String>,
    selected: usize,
    status: String,
}

impl Graph {
    pub fn new(db: Database) -> Self {
        let mut g = Self {
            db,
            concepts: Vec::new(),
            selected: 0,
            status: "GRAPH READY. Use ↑/↓. Navigation via command mode ':' -> c/d/g/q".to_string(),
        };
        g.refresh();
        g
    }

    fn refresh(&mut self) {
        match self.db.list_concept_names(500) {
            Ok(list) => {
                self.concepts = list;
                if self.selected >= self.concepts.len() {
                    self.selected = self.concepts.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("DB error: {}", e),
        }
    }

    fn selected_name(&self) -> Option<&str> {
        self.concepts.get(self.selected).map(|s| s.as_str())
    }
}

impl Module for Graph {
    fn render(&mut self, f: &mut Frame, status: &str) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(f.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        // Header / status
        let header_text = format!("{} | {}", self.status, status);
        let header = Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("MOTHER / GRAPH"),
        );
        f.render_widget(header, chunks[0]);

        // Left: concept list
        let items: Vec<ListItem> = self
            .concepts
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if i == self.selected {
                    ListItem::new(format!("> {}", name))
                } else {
                    ListItem::new(format!("  {}", name))
                }
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title("CONCEPTS"));

        f.render_widget(list, body[0]);

        // Right: relations for selected concept
        let right_text = if let Some(name) = self.selected_name() {
            match build_details(&self.db, name) {
                Ok(txt) => txt,
                Err(e) => format!("DB error: {}\n", e),
            }
        } else {
            "No concepts found.\nGo to DIALOG and add one using:\nlearn <concept> is <definition>\n"
                .to_string()
        };

        let rel_view = Paragraph::new(right_text)
            .block(Block::default().borders(Borders::ALL).title("RELATIONS"));

        f.render_widget(rel_view, body[1]);
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < self.concepts.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
    }
}

fn build_details(db: &Database, name: &str) -> Result<String> {
    let mut out = String::new();
    if let Some(concept) = db.get_concept(name)? {
        out.push_str(&format!("FOCUS: {}\n", concept.name));
        out.push_str(&format!("Definition: {}\n", concept.definition));
        out.push_str(&format!("Confidence: {:.2}\n", concept.confidence));
        out.push_str(&format!("Created: {}\n\n", concept.created_at));
    } else {
        out.push_str(&format!("Concept '{}' not found.\n", name));
    }

    let rels = db.list_relations_for(name, 200)?;
    out.push_str(&render_relations(name, &rels));
    out.push('\n');

    let claims = db.list_claims_for(name, 100)?;
    if claims.is_empty() {
        out.push_str("Claims: none\n");
    } else {
        out.push_str("Claims:\n");
        let mut evidence_cache = std::collections::HashMap::new();
        for claim in &claims {
            if let Some(eid) = claim.evidence_id {
                if !evidence_cache.contains_key(&eid) {
                    if let Ok(Some(ev)) = db.get_evidence(eid) {
                        evidence_cache.insert(eid, ev);
                    }
                }
            }
        }
        for claim in claims {
            let evidence_note = if let Some(eid) = claim.evidence_id {
                if let Some(ev) = evidence_cache.get(&eid) {
                    format!(" [evidence {}: {}]", eid, ev.source)
                } else {
                    format!(" [evidence {} missing]", eid)
                }
            } else {
                "".to_string()
            };
            out.push_str(&format!(
                "  ({:.2}) {}{}\n",
                claim.confidence, claim.claim_text, evidence_note
            ));
        }
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn render_relations(name: &str, rels: &[Relation]) -> String {
    let mut out = String::new();
    out.push_str(&format!("FOCUS: {}\n\n", name));
    if rels.is_empty() {
        out.push_str("No relations.\n\nAdd one in DIALOG like:\n  rel jwt uses jws\n  rel jwt used_for authentication\n");
        return out;
    }

    out.push_str("Outgoing:\n");
    for r in rels.iter().filter(|r| r.from == name) {
        out.push_str(&format!("  {} --{}--> {}\n", r.from, r.relation_type, r.to));
    }
    out.push('\n');
    out.push_str("Incoming:\n");
    for r in rels.iter().filter(|r| r.to == name) {
        out.push_str(&format!("  {} --{}--> {}\n", r.from, r.relation_type, r.to));
    }
    out
}
