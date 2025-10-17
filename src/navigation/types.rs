use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the different editing modes available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditingMode {
    /// Vim-style editing with normal/insert modes
    Vim,
    /// Standard cursor-based editing
    Cursor,
}

/// Represents the current mode within vim editing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    /// Normal mode for navigation and commands
    Normal,
    /// Insert mode for text input
    Insert,
    /// Visual mode for text selection
    Visual,
    /// Command mode for entering commands
    Command,
}

/// Represents a pane in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Connections,
    QueryInput,
    Results,
    SchemaExplorer,
    CommandLine,
}

impl Default for Pane {
    fn default() -> Self {
        Pane::Connections
    }
}

/// Represents a box within a pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Box {
    /// Text input box (can be edited)
    TextInput,
    /// Data table (view mode by default, can enter edit mode)
    DataTable,
    /// Tree view (navigation only)
    TreeView,
    /// List view (navigation only)
    ListView,
    /// Modal dialog
    Modal,
}

/// Represents navigation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Represents a navigation action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    /// Move in a direction
    Move(Direction),
    /// Focus a specific pane
    FocusPane(Pane),
    /// Focus a specific box within current pane
    FocusBox(Box),
    /// Switch to next pane
    NextPane,
    /// Switch to previous pane
    PreviousPane,
    /// Switch to next box within current pane
    NextBox,
    /// Switch to previous box within current pane
    PreviousBox,
    /// Enter edit mode for current box
    EnterEditMode,
    /// Exit edit mode
    ExitEditMode,
    /// Toggle between view and edit mode
    ToggleMode,
}

/// Configuration for navigation hotkeys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationConfig {
    /// Hotkey mappings for pane focusing
    pub pane_hotkeys: HashMap<char, Pane>,
    /// Hotkey mappings for box focusing within panes
    pub box_hotkeys: HashMap<char, Box>,
    /// Default editing mode
    pub default_editing_mode: EditingMode,
    /// Vim mode configuration
    pub vim_config: VimConfig,
    /// Cursor mode configuration
    pub cursor_config: CursorConfig,
}

/// Vim-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VimConfig {
    /// Key to enter insert mode
    pub insert_key: char,
    /// Key to enter visual mode
    pub visual_key: char,
    /// Key to enter command mode
    pub command_key: char,
    /// Key to exit current mode (usually Escape)
    pub exit_key: KeyCode,
    /// Whether to show mode indicator
    pub show_mode_indicator: bool,
}

/// Cursor-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorConfig {
    /// Whether to show cursor in text fields
    pub show_cursor: bool,
    /// Cursor style for different modes
    pub cursor_style: CursorStyle,
}

/// Cursor style options
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CursorStyle {
    Block,
    Bar,
    Underline,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        let mut pane_hotkeys = HashMap::new();
        pane_hotkeys.insert('c', Pane::Connections);
        pane_hotkeys.insert('q', Pane::QueryInput);
        pane_hotkeys.insert('r', Pane::Results);
        pane_hotkeys.insert('s', Pane::SchemaExplorer);
        pane_hotkeys.insert(':', Pane::CommandLine);

        let mut box_hotkeys = HashMap::new();
        box_hotkeys.insert('t', Box::TextInput);
        box_hotkeys.insert('d', Box::DataTable);
        box_hotkeys.insert('v', Box::TreeView);
        box_hotkeys.insert('l', Box::ListView);

        Self {
            pane_hotkeys,
            box_hotkeys,
            default_editing_mode: EditingMode::Vim,
            vim_config: VimConfig::default(),
            cursor_config: CursorConfig::default(),
        }
    }
}

impl Default for VimConfig {
    fn default() -> Self {
        Self {
            insert_key: 'i',
            visual_key: 'v',
            command_key: ':',
            exit_key: KeyCode::Esc,
            show_mode_indicator: true,
        }
    }
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            show_cursor: true,
            cursor_style: CursorStyle::Bar,
        }
    }
}

/// Represents the current navigation state
#[derive(Debug, Clone)]
pub struct NavigationState {
    /// Currently focused pane
    pub active_pane: Pane,
    /// Currently focused box within the pane
    pub active_box: Option<Box>,
    /// Current editing mode
    pub editing_mode: EditingMode,
    /// Current vim mode (if using vim editing)
    pub vim_mode: VimMode,
    /// Whether we're in view mode (for boxes that support it)
    pub view_mode: bool,
    /// Cursor position (row, column)
    pub cursor_position: (usize, usize),
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            active_pane: Pane::default(),
            active_box: None,
            editing_mode: EditingMode::Vim,
            vim_mode: VimMode::Normal,
            view_mode: true,
            cursor_position: (0, 0),
        }
    }
}