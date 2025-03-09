# LazyLode

A terminal-based database explorer for PostgreSQL and MongoDB.

## Installation

1. Ensure you have Rust installed (1.70.0 or later)
2. Clone the repository
3. Build and install:
```bash
cargo build --release
cargo install --path .
```

## Configuration

Configuration files are stored in `~/.config/lazylode/`:

- `config.toml`: Main configuration file
- `themes/`: Theme files directory
- `logs/`: Log files directory

### Default Config Structure

```toml
theme = "catppuccin_mocha"
[database]
default_port_postgres = 5432
default_port_mongodb = 27017

[connections]
# Your saved connections will be stored here
```

## Usage

Launch with:
```bash
lazylode
```

### Basic Navigation

- Arrow keys or hjkl: Navigate
- Shift+c: Focus connections pane
- Shift+q: Focus query pane
- Shift+d: Focus results pane
- Enter: Expand/select item
- Left/Right: Collapse/expand tree items

### Motion Commands

- y: yank cell
- yy: yank row

 Numeric modifiers (e.g. `3j` for down 3 rows) are also enabled.

### Connection Management

- a: Add new connection
- e: Edit connection
- d: Delete connection
- In connection form:
  - Tab/Up/Down: Navigate fields
  - Enter: Save connection
  - Esc: Cancel

### Query Interface

- WHERE clause: Filter conditions
- ORDER BY: Sorting criteria
- i: Enter insert mode for editing
- Esc: Return to normal mode
- Enter: Execute query

### Results Navigation

- s: Sort by column
- g: First page
- G: Last page
- n: Next page
- p: Previous page

## Logs

Log files are stored in `~/.config/lazylode/logs/` with timestamp-based naming.

## Theme Customization

Create custom themes in `~/.config/lazylode/themes/` as TOML files:

```toml
transparent_backgrounds = false
base = [40, 42, 54]
surface0 = [30, 31, 40]
text = [248, 248, 242]
accent = [189, 147, 249]
```
