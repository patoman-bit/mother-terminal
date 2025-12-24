use std::io;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

mod app;
mod db;
mod modules;
mod ui;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let result = app::run();

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;

    if let Err(err) = result {
        eprintln!("{:?}", err);
    }
    Ok(())
}
