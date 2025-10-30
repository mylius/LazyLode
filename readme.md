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
- Shift+T: Focus connection tree pane
- Shift+F: Focus query input pane
- Shift+R: Focus results pane
- Shift+S: Focus schema explorer
- ':' Open command line, Esc to cancel
- '/': Focus WHERE input (search)
- Enter: Expand/select item
- Left/Right: Collapse/expand tree items

### Motion Commands

- y: yank cell
- yy: yank row
- p/P: paste

Numeric modifiers (e.g. `3j` for down 3 rows) are enabled in Normal mode.

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
- .: Next page (default)
- ,: Previous page (default)

### Foreign Key Jump

- Follow foreign key from the current cell to the referenced row/table using your pane modifier + follow key.
- Default: Shift + l (configurable).
- You can change the follow key in `~/.config/lazylode/config.toml`:
  ```toml
  [keymap]
  # Used together with pane_modifier (Shift/Ctrl/Alt)
  follow_fk_key = "l" # example: Shift+l will follow FK
  pane_modifier = "Shift"
  ```

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
