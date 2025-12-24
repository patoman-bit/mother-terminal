## MOTHER Terminal

MOTHER Terminal is a terminal user interface (TUI) prototype inspired by classic operator consoles. It offers two screens:

- **Console** — a static status view.
- **Dialog** — a simple command-driven interface backed by an embedded SQLite database.

The app uses [ratatui](https://docs.rs/ratatui) for layout, [crossterm](https://docs.rs/crossterm) for terminal control, and [rusqlite](https://docs.rs/rusqlite) with the bundled SQLite driver.

### Requirements

- Rust toolchain (Rust 1.80+ recommended) with `cargo`.

### Running

```bash
cargo run
```

Controls:

- `d` — switch to the Dialog view  
- `c` — return to the Console view  
- `q` — quit the application

### Dialog commands

- `learn <concept> is <definition>` — stage a concept proposal (confirm with `y`, reject with `n`).  
- `show <concept>` — display a stored concept record.  
- `list` — show recent concepts in the database.

### Testing and linting

```bash
cargo test     # run tests
cargo clippy   # run lints
cargo fmt      # format code
```

### Notes

- The SQLite database file defaults to `mother.db` in the repository root. It is created automatically when the app starts.
- Dialog history is trimmed after 200 lines to keep the UI responsive.
