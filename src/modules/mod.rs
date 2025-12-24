use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

pub trait Module {
    fn render(&mut self, f: &mut Frame, area: Rect);
    fn handle_input(&mut self, key: KeyEvent);
}

pub mod console;
pub mod dialog;
pub mod graph;
