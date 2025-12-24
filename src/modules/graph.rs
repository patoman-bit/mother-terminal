use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::Module;
use crate::db::{Claim, Database, Evidence, Relation};

pub struct Graph {
    db: Database,
    concepts: Vec<String>,
    selected: usize,
    status: String,
    relations: Vec<Relation>,
    evidence: Vec<Evidence>,
    claims: Vec<Claim>,
}

impl Graph {
    pub fn new(db: Database) -> Self {
        let mut g = Self {
            db,
            concepts: Vec::new(),
            selected: 0,
            status: "GRAPH READY. Use ↑/↓. [Ctrl+C] CONSOLE [Ctrl+D] DIALOG [Ctrl+Q] QUIT"
                .to_string(),
            relations: Vec::new(),
            evidence: Vec::new(),
            claims: Vec::new(),
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
                self.load_selected_details();
            }
            Err(e) => self.status = format!("DB error: {}", e),
        }
    }

    fn load_selected_details(&mut self) {
        let Some(name) = self.selected_name().map(|s| s.to_string()) else {
            self.relations.clear();
            self.evidence.clear();
            self.claims.clear();
            return;
        };

        match self.db.list_relations_for(&name, 200) {
            Ok(items) => self.relations = items,
            Err(e) => {
                self.status = format!("DB error: {}", e);
                return;
            }
        }

        match self.db.list_evidence_for(&name, 50) {
            Ok(items) => self.evidence = items,
            Err(e) => {
                self.status = format!("DB error: {}", e);
                return;
            }
        }

        match self.db.list_claims_for(&name, 50) {
            Ok(items) => self.claims = items,
            Err(e) => {
                self.status = format!("DB error: {}", e);
            }
        }
    }

    fn selected_name(&self) -> Option<&str> {
        self.concepts.get(self.selected).map(|s| s.as_str())
    }
}

impl Module for Graph {
    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(f.area());

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
        let right_sections = if let Some(name) = self.selected_name() {
            (
                render_relations(name, &self.relations),
                render_evidence(name, &self.evidence),
                render_claims(name, &self.claims),
            )
        } else {
            let empty_text = "No concepts found.\nGo to DIALOG and add one using:\nlearn <concept> is <definition>\n".to_string();
            (empty_text.clone(), empty_text.clone(), empty_text)
        };

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Percentage(27),
                Constraint::Percentage(28),
            ])
            .split(body[1]);

        let rel_view = Paragraph::new(right_sections.0)
            .block(Block::default().borders(Borders::ALL).title("RELATIONS"));
        f.render_widget(rel_view, right_layout[0]);

        let evidence_view = Paragraph::new(right_sections.1)
            .block(Block::default().borders(Borders::ALL).title("EVIDENCE"));
        f.render_widget(evidence_view, right_layout[1]);

        let claims_view = Paragraph::new(right_sections.2)
            .block(Block::default().borders(Borders::ALL).title("CLAIMS"));
        f.render_widget(claims_view, right_layout[2]);
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.load_selected_details();
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < self.concepts.len() {
                    self.selected += 1;
                    self.load_selected_details();
                }
            }
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
    }
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

fn render_evidence(name: &str, evidence: &[Evidence]) -> String {
    let mut out = String::new();
    out.push_str(&format!("EVIDENCE FOR {}\n\n", name));
    if evidence.is_empty() {
        out.push_str("No evidence stored.\n\nCapture evidence in DIALOG, then refresh with [r].");
        return out;
    }

    for ev in evidence {
        out.push_str(&format!("• {}\n", ev.summary));
    }
    out
}

fn render_claims(name: &str, claims: &[Claim]) -> String {
    let mut out = String::new();
    out.push_str(&format!("CLAIMS ABOUT {}\n\n", name));
    if claims.is_empty() {
        out.push_str("No claims stored.\n\nAdd claims in DIALOG, then refresh with [r].");
        return out;
    }

    for cl in claims {
        out.push_str(&format!("• {}\n", cl.statement));
    }
    out
}
