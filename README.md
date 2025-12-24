# Mother Terminal (TUI)

Terminal-only interface for operator-controlled knowledge capture. Navigation uses command mode (`:` then `c/d/g/q` + Enter) so typing inside DIALOG is never interrupted.

## Key commands (dialog)
- `learn <concept> is <definition>` -> proposal requiring `y/n`
- `rel <from> <type> <to>` -> proposal requiring `y/n`
- `ep ok|fail|note <summary>` -> proposal requiring `y/n`
- `src <url> :: <excerpt>` -> evidence proposal (`y/n`)
- `claim <concept> :: <claim text> :: <evidence_id optional>` -> proposal (`y/n`)
- `claims <concept>` / `evidence` / `episodes` / `list` / `show <concept>`
- `doctor` (tool readiness)
- `search <query>` -> permissioned; `keep <n>` or `keep all` -> evidence proposal (`y/n`)

## Command mode
- Press `:` to enter, type `c/d/g/q`, Enter to execute, Esc to cancel.
- Status banner shows `CMD: :<buffer>` while active.

## Test checklist
- `cargo fmt`
- `cargo build`
- `timeout 3 cargo run` (ensure TUI starts and exits via command mode `:q`)
