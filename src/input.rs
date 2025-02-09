use serde::{Deserialize, Serialize};
use crossterm::event::{KeyCode, KeyModifiers};
use crate::ui::types::{Direction, Pane};

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
    NextTab,
    PreviousTab,
    FirstPage,
    PreviousPage,
    NextPage,
    LastPage,

}

/// Defines the key configuration for different actions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyConfig {
    // Direction keys for navigation
    /// Key for moving left.
    pub left_key: char,     // Default: 'h'
    /// Key for moving right.
    pub right_key: char,    // Default: 'l'
    /// Key for moving up.
    pub up_key: char,       // Default: 'k'
    /// Key for moving down.
    pub down_key: char,     // Default: 'j'

    // Direct pane access keys
    /// Key to focus on the connections pane.
    pub connections_key: char,  // Default: 'c'
    /// Key to focus on the query pane.
    pub query_key: char,        // Default: 'q'
    /// Key to focus on the data/results pane.
    pub data_key: char,         // Default: 'd'
    /// Key to trigger sorting in results pane.
    pub sort_key: char,         // Default: 's'

    // Tab navigation keys
    /// Key to select the next result tab.
    pub next_tab_key: char,     // Default: 'n'
    /// Key to select the previous result tab.
    pub prev_tab_key: char,     // Default: 'p'

    /// Key for first page
    pub first_page_key: char,     // Default: 'g'
    /// Key for last page
    pub last_page_key: char,      // Default: 'G'
    /// Key for next page
    pub next_page_key: char,      // Default: 'n'
    /// Key for previous page
    pub prev_page_key: char,      // Default: 'p'

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

            pane_modifier: PaneModifier::Shift, // Shift key as default pane modifier
        }
    }
}

impl KeyConfig {
    /// Maps a key event to an `Action` based on the current key configuration.
    pub fn get_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        let is_pane_modifier = match self.pane_modifier { // Check if pane modifier is pressed
            PaneModifier::Ctrl => modifiers.contains(KeyModifiers::CONTROL),
            PaneModifier::Alt => modifiers.contains(KeyModifiers::ALT),
            PaneModifier::Shift => modifiers.contains(KeyModifiers::SHIFT),
        };

        // Handle Enter key for tree item expansion (without modifier)
        if code == KeyCode::Enter && !is_pane_modifier {
            return Some(Action::TreeAction(TreeAction::Expand));
        }

        match code {
            KeyCode::Char(c) => {
                if is_pane_modifier {
                    // Pane switching actions (only with modifier)
                    let c_lower = c.to_ascii_lowercase(); // Case-insensitive matching
                    match c_lower {
                        c if c == self.connections_key.to_ascii_lowercase() =>
                            Some(Action::Navigation(NavigationAction::FocusPane(Pane::Connections))),
                        c if c == self.query_key.to_ascii_lowercase() =>
                            Some(Action::Navigation(NavigationAction::FocusPane(Pane::QueryInput))),
                        c if c == self.data_key.to_ascii_lowercase() =>
                            Some(Action::Navigation(NavigationAction::FocusPane(Pane::Results))),
                        _ => None,
                    }
                } else {
                    // Normal mode actions (without modifier)
                    match c {
                    c if c == self.last_page_key => Some(Action::LastPage),
                    c if c == self.next_page_key => Some(Action::NextPage),
                    c if c == self.prev_page_key => Some(Action::PreviousPage),
                    c if c == self.sort_key => Some(Action::Sort),
                    c if c == self.next_tab_key => Some(Action::NextTab),
                    c if c == self.prev_tab_key => Some(Action::PreviousTab),
                    c if c == self.left_key => Some(Action::Navigation(NavigationAction::Direction(Direction::Left))),
                    c if c == self.right_key => Some(Action::Navigation(NavigationAction::Direction(Direction::Right))),
                    c if c == self.up_key => Some(Action::Navigation(NavigationAction::Direction(Direction::Up))),
                    c if c == self.down_key => Some(Action::Navigation(NavigationAction::Direction(Direction::Down))),
                    _ => None
                }
            }
            }
            KeyCode::Left => Some(Action::TreeAction(TreeAction::Collapse)), // Collapse tree item
            KeyCode::Right => Some(Action::TreeAction(TreeAction::Expand)), // Expand tree item
            KeyCode::Up => Some(Action::Navigation(NavigationAction::Direction(Direction::Up))), // Move Up
            KeyCode::Down => Some(Action::Navigation(NavigationAction::Direction(Direction::Down))), // Move Down
            _ => None, // No action for other keys
        }
    }
}
