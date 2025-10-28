## LazyLode in Cursor

A fast TUI database explorer for PostgreSQL and MongoDB.

## Quickstart
- Requirements: Rust 1.70+
- Build (release):
```bash
cargo build --release
```
- Install locally:
```bash
cargo install --path .
```
- Run (dev):
```bash
cargo run
```
- Launch installed binary:
```bash
lazylode
```

## Configuration
- Config dir: `~/.config/lazylode/`
  - `config.toml` (main)
  - `themes/` (theme files)
  - `logs/` (runtime logs)
- Examples: `example_config.toml`, built-in themes in `config/themes/`.

## Dev workflow
- Format: `cargo fmt`
- Lint: `cargo clippy -- -D warnings`
- Run: `cargo run`
- Logs: `~/.config/lazylode/logs/`

## Repo map
- `src/main.rs`: entrypoint; logging + run loop
- `src/app.rs`: application state and core behaviors
- `src/ui/`: rendering, panes, modals, components
- `src/navigation/`: input handling, navigation, modes
- `src/database/`: postgres, mongodb, ssh tunnel
- `src/command.rs`: command registry + fuzzy suggestions
- Infra: `src/bootstrap.rs`, `src/runtime.rs`, `src/logging.rs`, `src/config.rs`, `src/theme.rs`

## Common edit points
- Add command: `register_default_commands` in `src/command.rs`
- Key/input behavior: `src/navigation/input_handler.rs`
- Key mappings (central): `src/navigation/types.rs` (`KeyMapping::default`, `NavigationAction`)
- Modals/UI: `src/ui/modals/*`, panes in `src/ui/panes/*`
- Themes: `config/themes/*` (switch via `App::switch_theme`)

## Editing guidelines
- Prefer general key mappings in `src/navigation/types.rs`; avoid hardcoding keys elsewhere
- Be DRY; prefer small, generalizable solutions without over-abstraction
- Small, focused edits; avoid unrelated reformatting
- Preserve existing style/indentation (Rust 2021)
- Prefer clear names; keep comments minimal and purposeful
- Avoid new dependencies unless necessary
- Use `anyhow::Result` for errors; avoid panics
- After changes: `cargo fmt` and `cargo clippy -- -D warnings`
