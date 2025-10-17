# Navigation System Refactor

This document describes the new unified navigation system that replaces the fractured vim-style navigation with a more organized and configurable approach.

## Overview

The new navigation system provides:

1. **Unified Pane Management**: Each pane has a name and can be focused by hotkey
2. **Box-based Navigation**: Within each pane, there can be several boxes that can be navigated directionally
3. **Configurable Editing Modes**: Support for both vim-style and cursor-based editing
4. **View/Edit Mode Switching**: Boxes can have view mode (navigation only) and edit mode
5. **Directional Navigation**: Move between panes and boxes using directional keys

## Architecture

### Core Components

- **NavigationManager**: Main coordinator for pane and box navigation
- **BoxManager**: Manages boxes within panes and their editing states
- **VimEditor**: Handles vim-style text editing with normal/insert/visual/command modes
- **NavigationInputHandler**: Unified input handling that integrates with the new system

### Types

- **Pane**: Top-level containers (Connections, QueryInput, Results, SchemaExplorer, CommandLine)
- **Box**: Components within panes (TextInput, DataTable, TreeView, ListView, Modal)
- **EditingMode**: Vim or Cursor editing
- **VimMode**: Normal, Insert, Visual, Command modes for vim editing

## Configuration

### Navigation Configuration

```toml
[navigation]
default_editing_mode = "Vim"  # or "Cursor"

# Pane hotkeys (no modifier needed)
[navigation.pane_hotkeys]
c = "Connections"
q = "QueryInput"
r = "Results"
s = "SchemaExplorer"
":" = "CommandLine"

# Box hotkeys within panes
[navigation.box_hotkeys]
t = "TextInput"
d = "DataTable"
v = "TreeView"
l = "ListView"

# Vim configuration
[navigation.vim_config]
insert_key = 'i'
visual_key = 'v'
command_key = ':'
exit_key = "Esc"
show_mode_indicator = true

# Cursor configuration
[navigation.cursor_config]
show_cursor = true
cursor_style = "Bar"  # "Block", "Bar", or "Underline"
```

## Usage

### Pane Navigation

- **Hotkey Focus**: Press the configured hotkey to focus a pane (e.g., 'c' for Connections)
- **Directional Navigation**: Use arrow keys or vim keys (h/j/k/l) to move between panes
- **Next/Previous Pane**: Use configured keys to cycle through panes

### Box Navigation

- **Hotkey Focus**: Press the configured hotkey to focus a box within the current pane
- **Directional Navigation**: Use arrow keys or vim keys to move between boxes
- **Next/Previous Box**: Use configured keys to cycle through boxes

### Editing Modes

#### Vim Mode
- **Normal Mode**: Navigation and commands (default)
- **Insert Mode**: Text input (press 'i' or 'a')
- **Visual Mode**: Text selection (press 'v')
- **Command Mode**: Command entry (press ':')

#### Cursor Mode
- **View Mode**: Navigation only (for boxes that support it)
- **Edit Mode**: Text input (press 'e' to enter)

### Mode Switching

- **Enter Edit Mode**: Press 'e' to enter edit mode for editable boxes
- **Toggle Mode**: Press 'v' to toggle between view and edit mode
- **Exit Edit Mode**: Press 'Esc' to exit edit mode

## Key Bindings

### Default Pane Hotkeys
- `c`: Focus Connections pane
- `q`: Focus QueryInput pane
- `r`: Focus Results pane
- `s`: Focus SchemaExplorer pane
- `:`: Focus CommandLine pane

### Default Box Hotkeys
- `t`: Focus TextInput box
- `d`: Focus DataTable box
- `v`: Focus TreeView box
- `l`: Focus ListView box

### Navigation Keys
- `h`/`←`: Move left
- `j`/`↓`: Move down
- `k`/`↑`: Move up
- `l`/`→`: Move right

### Mode Keys
- `e`: Enter edit mode
- `v`: Toggle view/edit mode
- `Esc`: Exit edit mode

## Migration

The new system maintains backward compatibility with the existing keymap configuration while adding the new navigation features. The old input handlers are still available as fallbacks.

## Benefits

1. **Unified Experience**: Consistent navigation across all panes and boxes
2. **Configurable**: Easy to customize hotkeys and behavior
3. **Extensible**: Easy to add new panes and boxes
4. **Mode-aware**: Different editing modes for different types of content
5. **User-friendly**: Clear visual indicators of current mode and focus