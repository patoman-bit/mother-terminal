use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use super::Module;

pub struct Console;

impl Console {
    pub fn new() -> Self {
        Self
    }
}

impl Module for Console {
    fn render(&mut self, f: &mut Frame, status: &str) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(f.area());

        let header = Paragraph::new(status).block(Block::default().borders(Borders::ALL));

        let body = Paragraph::new(
            "STATUS: ONLINE\nDATABASE: CONNECTED\nMODE: OPERATOR CONTROLLED\n\nAwaiting command...",
        )
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, layout[0]);
        f.render_widget(body, layout[1]);
    }

    fn handle_input(&mut self, _key: KeyEvent) {}
}
