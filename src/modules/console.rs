use ratatui::{
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Direction, Constraint},
    Frame,
};
use crossterm::event::KeyEvent;

use super::Module;

pub struct Console;

impl Console {
    pub fn new() -> Self {
        Self
    }
}

impl Module for Console {
    fn render(&mut self, f: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(f.area());

        let header = Paragraph::new("MOTHER SYSTEM CONSOLE  |  [F2] DIALOG  [F3] GRAPH  [Ctrl+Q] QUIT")
            .block(Block::default().borders(Borders::ALL));

        let body = Paragraph::new(
            "STATUS: ONLINE\nDATABASE: CONNECTED\nMODE: OPERATOR CONTROLLED\n\nAwaiting command..."
        ).block(Block::default().borders(Borders::ALL));

        f.render_widget(header, layout[0]);
        f.render_widget(body, layout[1]);
    }

    fn handle_input(&mut self, _key: KeyEvent) {}
}
