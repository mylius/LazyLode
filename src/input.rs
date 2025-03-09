use crate::{
    logging,
    ui::types::{Direction, Pane},
};
use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

/// Represents modifier keys for pane switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum PaneModifier {
    /// Control key modifier.
    Ctrl,
    /// Alt key modifier.
    Alt,
    /// Shift key modifier.
    Shift,
}

/// Represents navigation actions within the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    /// Directional navigation (Left, Right, Up, Down).
    Direction(Direction),
    /// Focus on a specific pane.
    FocusPane(Pane),
    NextTab,
    PreviousTab,
}

/// Represents actions related to the connection tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeAction {
    /// Expand a tree item.
    Expand,
    /// Collapse a tree item.
    Collapse,
}

/// Represents all possible actions in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Navigation(NavigationAction),
    TreeAction(TreeAction),
    Sort,
    FirstPage,
    PreviousPage,
    NextPage,
    LastPage,
    Edit,
    Delete,
    Confirm,
    Cancel,
    CopyCell,
    CopyRow,
}

/// Defines the key configuration for different actions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyConfig {
    // Direction keys for navigation
    pub left_key: char,  // Default: 'h'
    pub right_key: char, // Default: 'l'
    pub up_key: char,    // Default: 'k'
    pub down_key: char,  // Default: 'j'

    // Direct pane access keys
    pub connections_key: char, // Default: 'c'
    pub query_key: char,       // Default: 'q'
    pub data_key: char,        // Default: 'd'
    //
    /// Key to trigger sorting in results pane.
    pub sort_key: char, // Default: 's'

    // Tab navigation keys
    pub next_tab_key: char, // Default: 'n'
    pub prev_tab_key: char, // Default: 'p'

    /// page navigation keys
    pub first_page_key: char, // Default: 'g'
    pub last_page_key: char, // Default: 'G'
    pub next_page_key: char, // Default: 'n'
    pub prev_page_key: char, // Default: 'p'
    //
    // Edit and delete keys
    pub edit_key: char,   // Default: 'e'
    pub delete_key: char, // Default: 'd'

    pub copy_key: char, // Default: 'y'

    /// Modifier key to use for pane switching (Ctrl, Alt, Shift).
    pub pane_modifier: PaneModifier,
}

impl Default for KeyConfig {
    /// Returns the default key configuration (Vim-style navigation, Shift-based pane switching).
    fn default() -> Self {
        Self {
            // Vim-style defaults for directional navigation
            left_key: 'h',
            right_key: 'l',
            up_key: 'k',
            down_key: 'j',

            // Pane access defaults
            connections_key: 'c',
            query_key: 'q',
            data_key: 'd',

            // Action defaults
            sort_key: 's',
            next_tab_key: 'n',
            prev_tab_key: 'p',

            first_page_key: 'g',
            last_page_key: 'G',
            next_page_key: ',',
            prev_page_key: '.',

            // Edit and delete keys
            edit_key: 'e',
            delete_key: 'd',

            copy_key: 'y',

            pane_modifier: PaneModifier::Shift,
        }
    }
}

impl KeyConfig {
    /// Maps a key event to an `Action` based on the current key configuration.
    pub fn get_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        let is_pane_modifier = match self.pane_modifier {
            PaneModifier::Ctrl => modifiers.contains(KeyModifiers::CONTROL),
            PaneModifier::Alt => modifiers.contains(KeyModifiers::ALT),
            PaneModifier::Shift => modifiers.contains(KeyModifiers::SHIFT),
        };

        match code {
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Esc => Some(Action::Cancel),
            KeyCode::Char(c) => {
                if is_pane_modifier {
                    // Pane switching actions (only with modifier)
                    let c_lower = c.to_ascii_lowercase(); // Case-insensitive matching
                    match c_lower {
                        c if c == self.connections_key.to_ascii_lowercase() => Some(
                            Action::Navigation(NavigationAction::FocusPane(Pane::Connections)),
                        ),
                        c if c == self.query_key.to_ascii_lowercase() => Some(Action::Navigation(
                            NavigationAction::FocusPane(Pane::QueryInput),
                        )),
                        c if c == self.data_key.to_ascii_lowercase() => Some(Action::Navigation(
                            NavigationAction::FocusPane(Pane::Results),
                        )),
                        _ => None,
                    }
                } else {
                    // Normal mode actions (without modifier)
                    match c {
                        c if c == self.last_page_key => Some(Action::LastPage),
                        c if c == self.next_page_key => Some(Action::NextPage),
                        c if c == self.prev_page_key => Some(Action::PreviousPage),
                        c if c == self.sort_key => Some(Action::Sort),
                        c if c == self.next_tab_key => {
                            Some(Action::Navigation(NavigationAction::NextTab))
                        }
                        c if c == self.prev_tab_key => {
                            Some(Action::Navigation(NavigationAction::PreviousTab))
                        }
                        c if c == self.edit_key => Some(Action::Edit),
                        c if c == self.delete_key => Some(Action::Delete),
                        c if c == self.copy_key => Some(Action::CopyCell),
                        c if c == self.left_key => Some(Action::Navigation(
                            NavigationAction::Direction(Direction::Left),
                        )),
                        c if c == self.right_key => Some(Action::Navigation(
                            NavigationAction::Direction(Direction::Right),
                        )),
                        c if c == self.up_key => Some(Action::Navigation(
                            NavigationAction::Direction(Direction::Up),
                        )),
                        c if c == self.down_key => Some(Action::Navigation(
                            NavigationAction::Direction(Direction::Down),
                        )),
                        _ => None,
                    }
                }
            }
            KeyCode::Left => Some(Action::TreeAction(TreeAction::Collapse)),
            KeyCode::Right => Some(Action::TreeAction(TreeAction::Expand)),
            KeyCode::Up => Some(Action::Navigation(NavigationAction::Direction(
                Direction::Up,
            ))),
            KeyCode::Down => Some(Action::Navigation(NavigationAction::Direction(
                Direction::Down,
            ))),
            _ => None,
        }
    }
}
