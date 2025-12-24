use crossterm::event::KeyEvent;
use ratatui::Frame;

pub trait Module {
    fn render(&mut self, f: &mut Frame, status: &str);
    fn handle_input(&mut self, key: KeyEvent);
}

pub mod console;
pub mod dialog;
pub mod graph;
