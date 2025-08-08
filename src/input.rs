use crate::ui::types::{Direction, Pane};
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
    FollowForeignKey,
}

/// Defines the key configuration for different actions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct KeyConfig {
    // Direction keys for navigation
    #[serde(default = "default_left_key")]
    pub left_key: char, // Default: 'h'
    #[serde(default = "default_right_key")]
    pub right_key: char, // Default: 'l'
    #[serde(default = "default_up_key")]
    pub up_key: char, // Default: 'k'
    #[serde(default = "default_down_key")]
    pub down_key: char, // Default: 'j'

    // Direct pane access keys
    #[serde(default = "default_connections_key")]
    pub connections_key: char, // Default: 'c'
    #[serde(default = "default_query_key")]
    pub query_key: char, // Default: 'q'
    #[serde(default = "default_data_key")]
    pub data_key: char, // Default: 'd'
    //
    /// Key to trigger sorting in results pane.
    #[serde(default = "default_sort_key")]
    pub sort_key: char, // Default: 's'

    // Tab navigation keys
    #[serde(default = "default_next_tab_key")]
    pub next_tab_key: char, // Default: 'n'
    #[serde(default = "default_prev_tab_key")]
    pub prev_tab_key: char, // Default: 'p'

    /// page navigation keys
    #[serde(default = "default_first_page_key")]
    pub first_page_key: char, // Default: 'g'
    #[serde(default = "default_last_page_key")]
    pub last_page_key: char, // Default: 'G'
    #[serde(default = "default_next_page_key")]
    pub next_page_key: char, // Default: 'n'
    #[serde(default = "default_prev_page_key")]
    pub prev_page_key: char, // Default: 'p'
    //
    // Edit and delete keys
    #[serde(default = "default_edit_key")]
    pub edit_key: char, // Default: 'e'
    #[serde(default = "default_delete_key")]
    pub delete_key: char, // Default: 'd'

    #[serde(default = "default_copy_key")]
    pub copy_key: char, // Default: 'y'

    /// Modifier key to use for pane switching (Ctrl, Alt, Shift).
    #[serde(default = "default_pane_modifier")]
    pub pane_modifier: PaneModifier,

    /// Key to follow a foreign key (used with pane_modifier). Default provided if omitted in config
    #[serde(default = "default_follow_fk_key")]
    pub follow_fk_key: char,
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

            follow_fk_key: 'l',
        }
    }
}

fn default_follow_fk_key() -> char {
    'l'
}

fn default_left_key() -> char {
    'h'
}
fn default_right_key() -> char {
    'l'
}
fn default_up_key() -> char {
    'k'
}
fn default_down_key() -> char {
    'j'
}
fn default_connections_key() -> char {
    'c'
}
fn default_query_key() -> char {
    'q'
}
fn default_data_key() -> char {
    'd'
}
fn default_sort_key() -> char {
    's'
}
fn default_next_tab_key() -> char {
    'n'
}
fn default_prev_tab_key() -> char {
    'p'
}
fn default_first_page_key() -> char {
    'g'
}
fn default_last_page_key() -> char {
    'G'
}
fn default_next_page_key() -> char {
    ','
}
fn default_prev_page_key() -> char {
    '.'
}
fn default_edit_key() -> char {
    'e'
}
fn default_delete_key() -> char {
    'd'
}
fn default_copy_key() -> char {
    'y'
}
fn default_pane_modifier() -> PaneModifier {
    PaneModifier::Shift
}

impl KeyConfig {
    /// Maps a key event to an `Action` based on the current key configuration.
    pub fn get_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        match code {
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Esc => Some(Action::Cancel),
            KeyCode::Char(c) => {
                // Determine if pane modifier is effectively active for pane/tab keys only.
                // Shift may be encoded as uppercase without the SHIFT flag in some terminals.
                let c_lower = c.to_ascii_lowercase();
                let is_pane_related_key = c_lower == self.connections_key.to_ascii_lowercase()
                    || c_lower == self.query_key.to_ascii_lowercase()
                    || c_lower == self.data_key.to_ascii_lowercase()
                    || c_lower == self.next_tab_key.to_ascii_lowercase()
                    || c_lower == self.prev_tab_key.to_ascii_lowercase()
                    || c_lower == self.follow_fk_key.to_ascii_lowercase();

                let is_pane_modifier_for_char = match self.pane_modifier {
                    PaneModifier::Ctrl => {
                        modifiers.contains(KeyModifiers::CONTROL) && is_pane_related_key
                    }
                    PaneModifier::Alt => {
                        modifiers.contains(KeyModifiers::ALT) && is_pane_related_key
                    }
                    PaneModifier::Shift => {
                        (modifiers.contains(KeyModifiers::SHIFT) && is_pane_related_key)
                            || (c.is_ascii_uppercase() && is_pane_related_key)
                    }
                };

                if is_pane_modifier_for_char {
                    // Pane switching actions (only with modifier)
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
                        c if c == self.next_tab_key.to_ascii_lowercase() => {
                            Some(Action::Navigation(NavigationAction::NextTab))
                        }
                        c if c == self.prev_tab_key.to_ascii_lowercase() => {
                            Some(Action::Navigation(NavigationAction::PreviousTab))
                        }
                        c if c == self.follow_fk_key.to_ascii_lowercase() => {
                            Some(Action::FollowForeignKey)
                        }
                        _ => None,
                    }
                } else {
                    // Normal mode actions (without modifier)
                    match c {
                        c if c == self.first_page_key => Some(Action::FirstPage),
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
