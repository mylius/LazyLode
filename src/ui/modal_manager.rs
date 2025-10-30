use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{layout::Rect, Frame};
use std::fmt;

use crate::app::App;
use crate::navigation::types::NavigationAction;

/// Result of modal input handling
#[derive(Debug, Clone, PartialEq)]
pub enum ModalResult {
    /// Modal should be closed
    Closed,
    /// Modal handled input, continue processing
    Continue,
    /// Modal wants to perform an action
    Action(String),
}

/// Trait that all modals must implement
pub trait Modal: fmt::Debug {
    /// Render the modal
    fn render(&self, frame: &mut Frame, area: Rect, app: &App);

    /// Handle input for this modal
    fn handle_input(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
        nav_action: Option<crate::navigation::types::NavigationAction>,
    ) -> ModalResult;

    /// Get the modal's title for display
    fn get_title(&self) -> &str;

    /// Get the modal's current vim mode (if applicable)
    fn get_mode(&self) -> Option<crate::navigation::types::VimMode> {
        None
    }

    /// Whether this modal blocks interaction with underlying UI
    fn is_blocking(&self) -> bool {
        true
    }

    /// Get the modal's preferred size (width, height) as percentages
    fn get_size(&self) -> (u16, u16) {
        (60, 50) // Default size
    }

    /// Close this modal (called when modal should be removed from stack)
    fn close(&mut self) {
        // Default implementation does nothing
        // Override in specific modals if cleanup is needed
    }
}

/// Manages a stack of modals with LIFO behavior
pub struct ModalManager {
    pub stack: Vec<Box<dyn Modal>>,
}

impl ModalManager {
    /// Create a new modal manager
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a modal onto the stack
    pub fn push(&mut self, modal: Box<dyn Modal>) {
        self.stack.push(modal);
    }

    /// Pop the top modal from the stack
    pub fn pop(&mut self) -> Option<Box<dyn Modal>> {
        if let Some(mut modal) = self.stack.pop() {
            modal.close();
            Some(modal)
        } else {
            None
        }
    }

    /// Close the active modal (calls close() method and removes from stack)
    pub fn close_active(&mut self) -> Option<Box<dyn Modal>> {
        self.pop()
    }

    /// Check if there are any modals
    pub fn has_modals(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Check if the active modal blocks interaction
    pub fn active_blocks_interaction(&self) -> bool {
        self.stack.last().map(|m| m.is_blocking()).unwrap_or(false)
    }

    /// Get the title of the active modal
    pub fn get_active_title(&self) -> Option<String> {
        self.stack.last().map(|m| m.get_title().to_string())
    }

    /// Get the current vim mode from the active modal
    pub fn get_active_mode(&self) -> Option<crate::navigation::types::VimMode> {
        self.stack.last().and_then(|m| m.get_mode())
    }

    /// Check if a modal with the given title is already open
    pub fn has_modal_with_title(&self, title: &str) -> bool {
        self.stack.iter().any(|m| m.get_title() == title)
    }

    /// Bring a modal with the given title to the front (if it exists)
    pub fn focus_modal_with_title(&mut self, title: &str) -> bool {
        if let Some(index) = self.stack.iter().position(|m| m.get_title() == title) {
            let modal = self.stack.remove(index);
            self.stack.push(modal);
            true
        } else {
            false
        }
    }

    /// Handle input for the active modal
    pub fn handle_input(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
        nav_action: Option<crate::navigation::types::NavigationAction>,
    ) -> Option<ModalResult> {
        if let Some(modal) = self.stack.last_mut() {
            Some(modal.handle_input(key, modifiers, nav_action))
        } else {
            None
        }
    }

    /// Render all modals in the stack
    pub fn render_all(&self, frame: &mut Frame, app: &App) {
        for modal in &self.stack {
            let (width, height) = modal.get_size();
            let area = Self::centered_rect_static(width, height, frame.area());
            modal.render(frame, area, app);
        }
    }

    fn centered_rect_static(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        use ratatui::layout::{Constraint, Direction, Layout};

        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    /// Calculate a centered rectangle for modal rendering
    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        Self::centered_rect_static(percent_x, percent_y, r)
    }

    /// Clear all modals
    pub fn clear(&mut self) {
        self.stack.clear();
    }

    /// Get the number of modals in the stack
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Check if the stack is empty
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl Default for ModalManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Common modal input handling utilities
pub mod utils {
    use super::*;

    /// Handle common modal keys using key mappings
    pub fn handle_common_keys(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &App,
    ) -> Option<ModalResult> {
        if let Some(action) = app
            .navigation_manager
            .config()
            .key_mapping
            .get_action(key, modifiers)
        {
            match action {
                NavigationAction::Cancel => Some(ModalResult::Closed),
                NavigationAction::Quit => Some(ModalResult::Closed),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Handle quit action using key mappings
    pub fn handle_quit_action(
        key: KeyCode,
        modifiers: KeyModifiers,
        app: &App,
    ) -> Option<ModalResult> {
        if let Some(action) = app
            .navigation_manager
            .config()
            .key_mapping
            .get_action(key, modifiers)
        {
            match action {
                NavigationAction::Quit => Some(ModalResult::Closed),
                _ => None,
            }
        } else {
            None
        }
    }
}
