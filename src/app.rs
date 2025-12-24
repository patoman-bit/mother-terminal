use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};
use std::{error::Error, io};

use crate::db::Database;
use crate::modules::{Module, console::Console, dialog::Dialog, graph::Graph};

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
    command_mode: bool,
    command_buffer: String,
    status: String,
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
        status: "Welcome. Press ':' for command mode. Esc to cancel.".to_string(),
    };

    loop {
        terminal.draw(|f| {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(f.area());

            let header = Paragraph::new(app.header_text())
                .block(Block::default().borders(Borders::ALL).title("MOTHER"));
            f.render_widget(header, layout[0]);

            match app.screen {
                Screen::Console => app.console.render(f, layout[1]),
                Screen::Dialog => app.dialog.render(f, layout[1]),
                Screen::Graph => app.graph.render(f, layout[1]),
            };
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if app.command_mode {
                    match key.code {
                        KeyCode::Esc => app.exit_command_mode("Command mode cancelled."),
                        KeyCode::Enter => {
                            let cmd = app.command_buffer.trim().to_string();
                            app.exit_command_mode("");
                            if !app.handle_command(&cmd)? {
                                return Ok(());
                            }
                        }
                        KeyCode::Backspace => {
                            app.command_buffer.pop();
                            app.status = format!("CMD: :{}", app.command_buffer);
                        }
                        KeyCode::Char(c) => {
                            app.command_buffer.push(c);
                            app.status = format!("CMD: :{}", app.command_buffer);
                        }
                        _ => {}
                    }
                    continue;
                }

                if matches!(key.code, KeyCode::Char(':')) {
                    app.command_mode = true;
                    app.command_buffer.clear();
                    app.status = "CMD: :".to_string();
                    continue;
                }

                // Normal typing goes to active module.
                match app.screen {
                    Screen::Console => app.console.handle_input(key),
                    Screen::Dialog => app.dialog.handle_input(key),
                    Screen::Graph => app.graph.handle_input(key),
                };
            }
        }
    }
}

impl App {
    fn header_text(&self) -> String {
        let screen = match self.screen {
            Screen::Console => "CONSOLE",
            Screen::Dialog => "DIALOG",
            Screen::Graph => "GRAPH",
        };

        let mode = if self.command_mode {
            format!("CMD: :{}", self.command_buffer)
        } else {
            "MODE: INPUT".to_string()
        };

        format!("Screen: {} | {} | {}", screen, mode, self.status)
    }

    fn exit_command_mode(&mut self, status: &str) {
        self.command_mode = false;
        self.command_buffer.clear();
        if !status.is_empty() {
            self.status = status.to_string();
        } else {
            self.status = "Command mode exited.".to_string();
        }
    }

    /// Returns false if the command requests quitting.
    fn handle_command(&mut self, command: &str) -> Result<bool, Box<dyn Error>> {
        match command {
            "c" => {
                self.screen = Screen::Console;
                self.status = "Switched to CONSOLE.".to_string();
            }
            "d" => {
                self.screen = Screen::Dialog;
                self.status = "Switched to DIALOG.".to_string();
            }
            "g" => {
                self.screen = Screen::Graph;
                self.status = "Switched to GRAPH.".to_string();
            }
            "q" => return Ok(false),
            "" => {
                self.status = "No command entered.".to_string();
            }
            other => {
                self.status = format!("Unknown command: {}", other);
            }
        }
        Ok(true)
    }
}
