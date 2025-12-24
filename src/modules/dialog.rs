use ratatui::{
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Direction, Constraint},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};

use super::Module;
use crate::db::Database;

pub struct Dialog {
    input: String,
    history: Vec<String>,
    _db: Database,
}

impl Dialog {
    pub fn new(db: Database) -> Self {
        Self {
            input: String::new(),
            history: vec!["MOTHER: Tell me what is on your mind.".into()],
            _db: db,
        }
    }
}

impl Module for Dialog {
    fn render(&mut self, f: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(3),
            ])
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
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Backspace => { self.input.pop(); }
            KeyCode::Enter => {
                let response = format!("MOTHER: Why do you say '{}'?",&self.input);
                self.history.push(format!("YOU: {}", self.input));
                self.history.push(response);
                self.input.clear();
            }
            _ => {}
        }
    }
}
