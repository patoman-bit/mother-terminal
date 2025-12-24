use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io;

mod app;
mod db;
mod modules;
mod search;
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
