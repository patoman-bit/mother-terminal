use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::Module;
use crate::db::{Database, Evidence};

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
            status: "GRAPH READY. Use ↑/↓. ':' enters command mode for navigation.".to_string(),
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
    fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        // Header / status
        let header = Paragraph::new(self.status.as_str()).block(
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
            render_details(&self.db, name)
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

fn render_details(db: &Database, name: &str) -> String {
    let mut out = String::new();
    match db.get_concept(name) {
        Ok(Some(c)) => {
            out.push_str(&format!("FOCUS: {}\n", c.name));
            out.push_str(&format!("Definition: {}\n", c.definition));
            out.push_str(&format!("Confidence: {:.2}\n", c.confidence));
            out.push_str(&format!("Created: {}\n\n", c.created_at));
        }
        Ok(None) => {
            out.push_str(&format!("Concept '{}' not found.\n", name));
            return out;
        }
        Err(e) => {
            out.push_str(&format!("DB error fetching concept: {}\n", e));
            return out;
        }
    }

    match db.list_relations_for(name, 200) {
        Ok(rels) if rels.is_empty() => {
            out.push_str("No relations yet. Add from DIALOG:\n  rel <from> <type> <to>\n\n");
        }
        Ok(rels) => {
            out.push_str("Outgoing:\n");
            for r in rels.iter().filter(|r| r.from == name) {
                out.push_str(&format!("  {} --{}--> {}\n", r.from, r.relation_type, r.to));
            }
            out.push('\n');
            out.push_str("Incoming:\n");
            for r in rels.iter().filter(|r| r.to == name) {
                out.push_str(&format!("  {} --{}--> {}\n", r.from, r.relation_type, r.to));
            }
            out.push('\n');
        }
        Err(e) => out.push_str(&format!("DB error: {}\n\n", e)),
    }

    match db.list_claims_for(name, 50) {
        Ok(claims) if claims.is_empty() => out.push_str("Claims: none\n\n"),
        Ok(claims) => {
            out.push_str("Claims:\n");
            for claim in claims {
                let mut line = format!(
                    "  [{}] {:.2} {}",
                    claim.id, claim.confidence, claim.statement
                );
                if let Some(evidence_id) = claim.evidence_id {
                    line.push_str(&format!(" (evidence #{})", evidence_id));
                    if let Ok(Some(ev)) = db.get_evidence(evidence_id) {
                        line.push_str(&format!(
                            "\n      evidence: {} :: {}",
                            ev.source, ev.content
                        ));
                    }
                }
                out.push_str(&line);
                out.push('\n');
            }
            out.push('\n');
        }
        Err(e) => out.push_str(&format!("DB error loading claims: {}\n\n", e)),
    }

    match db.list_evidence_for(name, 20) {
        Ok(evs) if evs.is_empty() => out.push_str("Evidence: none\n"),
        Ok(evs) => {
            out.push_str("Evidence (recent):\n");
            for Evidence {
                id,
                source,
                content,
                captured_at,
                ..
            } in evs
            {
                out.push_str(&format!(
                    "  [{}] {} :: {} ({})\n",
                    id, source, content, captured_at
                ));
            }
        }
        Err(e) => out.push_str(&format!("DB error loading evidence: {}\n", e)),
    }

    out
}
