use crossterm::event::{self, Event, KeyCode};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{error::Error, io};

use crate::db::Database;
use crate::modules::{Module, console::Console, dialog::Dialog, graph::Graph};

#[derive(Debug)]
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
    pub status_line: String,
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
        command_mode: false,
        command_buffer: String::new(),
        status_line: "READY".to_string(),
    };

    loop {
        terminal.draw(|f| {
            let status = command_status(&app);
            match app.screen {
                Screen::Console => app.console.render(f, &status),
                Screen::Dialog => app.dialog.render(f, &status),
                Screen::Graph => app.graph.render(f, &status),
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if app.command_mode {
                    match handle_command_mode(&mut app, key)? {
                        CommandOutcome::Quit => return Ok(()),
                        CommandOutcome::Handled => continue,
                        CommandOutcome::Ignored => {}
                    }
                } else if key.code == KeyCode::Char(':') {
                    app.command_mode = true;
                    app.command_buffer.clear();
                    app.status_line = "CMD MODE".to_string();
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

fn command_status(app: &App) -> String {
    if app.command_mode {
        format!("CMD: :{}", app.command_buffer)
    } else {
        format!(
            "MODE: {:?} | CMD: idle (press ':' ) | {}",
            app.screen, app.status_line
        )
    }
}

enum CommandOutcome {
    Handled,
    Ignored,
    Quit,
}

fn handle_command_mode(
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<CommandOutcome, Box<dyn Error>> {
    match key.code {
        KeyCode::Esc => {
            app.command_mode = false;
            app.command_buffer.clear();
            app.status_line = "CMD canceled".to_string();
            return Ok(CommandOutcome::Handled);
        }
        KeyCode::Backspace => {
            app.command_buffer.pop();
            return Ok(CommandOutcome::Handled);
        }
        KeyCode::Enter => {
            let cmd = app.command_buffer.trim().to_lowercase();
            match cmd.as_str() {
                "q" => return Ok(CommandOutcome::Quit),
                "c" => {
                    app.screen = Screen::Console;
                    app.status_line = "Switched to CONSOLE".to_string();
                }
                "d" => {
                    app.screen = Screen::Dialog;
                    app.status_line = "Switched to DIALOG".to_string();
                }
                "g" => {
                    app.screen = Screen::Graph;
                    app.status_line = "Switched to GRAPH".to_string();
                }
                "" => {
                    app.status_line = "CMD empty".to_string();
                }
                other => {
                    app.status_line = format!("Unknown command: {}", other);
                }
            }
            app.command_mode = false;
            app.command_buffer.clear();
            return Ok(CommandOutcome::Handled);
        }
        KeyCode::Char(c) => {
            app.command_buffer.push(c);
            return Ok(CommandOutcome::Handled);
        }
        _ => {}
    }
    Ok(CommandOutcome::Ignored)
}
