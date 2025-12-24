use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::Module;
use crate::db::{Database, Relation};

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
            status: "GRAPH READY. Use ↑/↓. [Ctrl+C] CONSOLE [Ctrl+D] DIALOG [Ctrl+Q] QUIT".to_string(),
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
        let header = Paragraph::new(self.status.as_str())
            .block(Block::default().borders(Borders::ALL).title("MOTHER / GRAPH"));
        f.render_widget(header, chunks[0]);

        // Left: concept list
        let items: Vec<ListItem> = self.concepts
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

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("CONCEPTS"));

        f.render_widget(list, body[0]);

        // Right: relations for selected concept
        let right_text = if let Some(name) = self.selected_name() {
            match self.db.list_relations_for(name, 200) {
                Ok(rels) => render_relations(name, &rels),
                Err(e) => format!("DB error: {}\n", e),
            }
        } else {
            "No concepts found.\nGo to DIALOG and add one using:\nlearn <concept> is <definition>\n".to_string()
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
