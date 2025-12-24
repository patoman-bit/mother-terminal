use std::{error::Error, io};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
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
    pub command_mode: bool,
    pub command_buffer: String,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        screen: Screen::Console,
        console: Console::new(),
        dialog: Dialog::new(Database::init("mother.db")?),
        graph: Graph::new(Database::init("mother.db")?),
        command_mode: false,
        command_buffer: String::new(),
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
                // --- COMMAND MODE ---
                if app.command_mode {
                    match key.code {
                        KeyCode::Char(c) => app.command_buffer.push(c),
                        KeyCode::Backspace => { app.command_buffer.pop(); }
                        KeyCode::Enter => {
                            match app.command_buffer.as_str() {
                                "c" => app.screen = Screen::Console,
                                "d" => app.screen = Screen::Dialog,
                                "g" => app.screen = Screen::Graph,
                                "q" => return Ok(()),
                                _ => {}
                            }
                            app.command_buffer.clear();
                            app.command_mode = false;
                        }
                        KeyCode::Esc => {
                            app.command_buffer.clear();
                            app.command_mode = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // Enter command mode
                if let KeyCode::Char(':') = key.code {
                    app.command_mode = true;
                    app.command_buffer.clear();
                    continue;
                }

                // Normal input → active module
                match app.screen {
                    Screen::Console => app.console.handle_input(key),
                    Screen::Dialog => app.dialog.handle_input(key),
                    Screen::Graph => app.graph.handle_input(key),
                }
            }
        }
    }
}
