use std::{error::Error, io};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::modules::{Module, console::Console, dialog::Dialog};
use crate::db::Database;

pub enum Screen {
    Console,
    Dialog,
}

pub struct App {
    pub screen: Screen,
    pub console: Console,
    pub dialog: Dialog,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let db = Database::init("mother.db")?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        screen: Screen::Console,
        console: Console::new(),
        dialog: Dialog::new(db),
    };

    loop {
        terminal.draw(|f| {
            match app.screen {
                Screen::Console => app.console.render(f),
                Screen::Dialog => app.dialog.render(f),
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('d') => app.screen = Screen::Dialog,
                    KeyCode::Char('c') => app.screen = Screen::Console,
                    _ => match app.screen {
                        Screen::Console => app.console.handle_input(key),
                        Screen::Dialog => app.dialog.handle_input(key),
                    }
                }
            }
        }
    }
}
