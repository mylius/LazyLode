use crate::ui::types::{Direction as OldDirection, Pane as OldPane};
use crate::navigation::types::{Direction as NewDirection, Pane as NewPane, NavigationAction as NewNavigationAction};

/// Migration utilities to convert between old and new navigation types
pub struct NavigationMigration;

impl NavigationMigration {
    /// Convert old Direction to new Direction
    pub fn direction_old_to_new(direction: OldDirection) -> NewDirection {
        match direction {
            OldDirection::Left => NewDirection::Left,
            OldDirection::Right => NewDirection::Right,
            OldDirection::Up => NewDirection::Up,
            OldDirection::Down => NewDirection::Down,
        }
    }

    /// Convert new Direction to old Direction
    pub fn direction_new_to_old(direction: NewDirection) -> OldDirection {
        match direction {
            NewDirection::Left => OldDirection::Left,
            NewDirection::Right => OldDirection::Right,
            NewDirection::Up => OldDirection::Up,
            NewDirection::Down => OldDirection::Down,
        }
    }

    /// Convert old Pane to new Pane
    pub fn pane_old_to_new(pane: OldPane) -> NewPane {
        match pane {
            OldPane::Connections => NewPane::Connections,
            OldPane::QueryInput => NewPane::QueryInput,
            OldPane::Results => NewPane::Results,
        }
    }

    /// Convert new Pane to old Pane
    pub fn pane_new_to_old(pane: NewPane) -> OldPane {
        match pane {
            NewPane::Connections => OldPane::Connections,
            NewPane::QueryInput => OldPane::QueryInput,
            NewPane::Results => OldPane::Results,
            NewPane::SchemaExplorer => OldPane::Connections, // Map to closest equivalent
            NewPane::CommandLine => OldPane::QueryInput, // Map to closest equivalent
        }
    }

    /// Convert old NavigationAction to new NavigationAction
    pub fn navigation_action_old_to_new(action: crate::input::NavigationAction) -> NewNavigationAction {
        match action {
            crate::input::NavigationAction::Direction(direction) => {
                match Self::direction_old_to_new(direction) {
                    NewDirection::Left => NewNavigationAction::MoveLeft,
                    NewDirection::Right => NewNavigationAction::MoveRight,
                    NewDirection::Up => NewNavigationAction::MoveUp,
                    NewDirection::Down => NewNavigationAction::MoveDown,
                }
            }
            crate::input::NavigationAction::FocusPane(pane) => {
                match Self::pane_old_to_new(pane) {
                    NewPane::Connections => NewNavigationAction::FocusConnections,
                    NewPane::QueryInput => NewNavigationAction::FocusQueryInput,
                    NewPane::Results => NewNavigationAction::FocusResults,
                    NewPane::SchemaExplorer => NewNavigationAction::FocusSchemaExplorer,
                    NewPane::CommandLine => NewNavigationAction::FocusCommandLine,
                }
            }
            crate::input::NavigationAction::NextTab => {
                NewNavigationAction::NextBox
            }
            crate::input::NavigationAction::PreviousTab => {
                NewNavigationAction::PreviousBox
            }
        }
    }
}