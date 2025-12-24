use std::{error::Error, io};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::modules::{Module, console::Console, dialog::Dialog, graph::Graph};
use crate::db::Database;

pub enum Screen {
    Console,
    Dialog,
    Graph,
}

pub struct App {
    pub screen: Screen,
    pub console: Console,
    pub dialog: Dialog,
    pub graph: Graph,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // v0 simplicity: separate connections; later we’ll share one safely
    let mut app = App {
        screen: Screen::Console,
        console: Console::new(),
        dialog: Dialog::new(Database::init("mother.db")?),
        graph: Graph::new(Database::init("mother.db")?),
    };

    loop {
        terminal.draw(|f| {
            match app.screen {
                Screen::Console => app.console.render(f),
                Screen::Dialog => app.dialog.render(f),
                Screen::Graph => app.graph.render(f),
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only treat Ctrl+<key> as global command shortcuts.
                let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

                if is_ctrl {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('c') => app.screen = Screen::Console,
                        KeyCode::Char('d') => app.screen = Screen::Dialog,
                        KeyCode::Char('g') => app.screen = Screen::Graph,
                        _ => {}
                    }
                    continue;
                }

                // Otherwise: pass keystroke to current module (so typing works)
                match app.screen {
                    Screen::Console => app.console.handle_input(key),
                    Screen::Dialog => app.dialog.handle_input(key),
                    Screen::Graph => app.graph.handle_input(key),
                }
            }
        }
    }
}
