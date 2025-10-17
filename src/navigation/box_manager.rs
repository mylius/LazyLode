use crate::navigation::types::{Box, Pane, Direction, EditingMode};
use crate::navigation::vim_editor::VimEditor;
use crossterm::event::{KeyCode, KeyModifiers};

/// Manages boxes within panes and their editing states
pub struct BoxManager {
    /// Current active box
    active_box: Option<Box>,
    /// Vim editor for text input boxes
    vim_editor: VimEditor,
    /// Whether we're in view mode (for boxes that support it)
    view_mode: bool,
    /// Current editing mode
    editing_mode: EditingMode,
}

impl BoxManager {
    pub fn new() -> Self {
        Self {
            active_box: None,
            vim_editor: VimEditor::new(),
            view_mode: true,
            editing_mode: EditingMode::Vim,
        }
    }

    pub fn active_box(&self) -> Option<Box> {
        self.active_box
    }

    pub fn set_active_box(&mut self, box_type: Option<Box>) {
        self.active_box = box_type;
        if let Some(Box::TextInput) = box_type {
            self.view_mode = false; // Text input boxes are always in edit mode
        } else {
            self.view_mode = true; // Other boxes start in view mode
        }
    }

    pub fn view_mode(&self) -> bool {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, view_mode: bool) {
        self.view_mode = view_mode;
    }

    pub fn editing_mode(&self) -> EditingMode {
        self.editing_mode
    }

    pub fn set_editing_mode(&mut self, mode: EditingMode) {
        self.editing_mode = mode;
    }

    pub fn vim_editor(&self) -> &VimEditor {
        &self.vim_editor
    }

    pub fn vim_editor_mut(&mut self) -> &mut VimEditor {
        &mut self.vim_editor
    }

    /// Get the available boxes for a given pane
    pub fn get_available_boxes(&self, pane: Pane) -> Vec<Box> {
        match pane {
            Pane::Connections => vec![Box::TreeView],
            Pane::QueryInput => vec![Box::TextInput],
            Pane::Results => vec![Box::DataTable],
            Pane::SchemaExplorer => vec![Box::TreeView, Box::ListView],
            Pane::CommandLine => vec![Box::TextInput],
        }
    }

    /// Navigate to the next box in the current pane
    pub fn next_box(&mut self, pane: Pane) -> Option<Box> {
        let available_boxes = self.get_available_boxes(pane);
        if available_boxes.is_empty() {
            return None;
        }

        let current_index = self.active_box
            .and_then(|b| available_boxes.iter().position(|&box_type| box_type == b))
            .unwrap_or(0);

        let next_index = (current_index + 1) % available_boxes.len();
        let next_box = available_boxes[next_index];
        self.set_active_box(Some(next_box));
        Some(next_box)
    }

    /// Navigate to the previous box in the current pane
    pub fn previous_box(&mut self, pane: Pane) -> Option<Box> {
        let available_boxes = self.get_available_boxes(pane);
        if available_boxes.is_empty() {
            return None;
        }

        let current_index = self.active_box
            .and_then(|b| available_boxes.iter().position(|&box_type| box_type == b))
            .unwrap_or(0);

        let prev_index = if current_index == 0 {
            available_boxes.len() - 1
        } else {
            current_index - 1
        };

        let prev_box = available_boxes[prev_index];
        self.set_active_box(Some(prev_box));
        Some(prev_box)
    }

    /// Handle a key event for the current box
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        match self.active_box {
            Some(Box::TextInput) => {
                if self.editing_mode == EditingMode::Vim {
                    self.vim_editor.handle_key(key, modifiers)
                } else {
                    self.handle_cursor_mode_key(key, modifiers)
                }
            }
            Some(Box::DataTable) => {
                if self.view_mode {
                    self.handle_table_view_key(key, modifiers)
                } else {
                    self.handle_table_edit_key(key, modifiers)
                }
            }
            Some(Box::TreeView) | Some(Box::ListView) => {
                self.handle_navigation_key(key, modifiers)
            }
            Some(Box::Modal) => {
                self.handle_modal_key(key, modifiers)
            }
            None => false,
        }
    }

    fn handle_cursor_mode_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Char(c) => {
                self.vim_editor.insert_char_at_cursor(c);
                true
            }
            KeyCode::Backspace => {
                self.vim_editor.delete_char_before_cursor();
                true
            }
            KeyCode::Enter => {
                self.vim_editor.insert_newline();
                true
            }
            KeyCode::Left => {
                self.vim_editor.move_cursor(Direction::Left);
                true
            }
            KeyCode::Right => {
                self.vim_editor.move_cursor(Direction::Right);
                true
            }
            KeyCode::Up => {
                self.vim_editor.move_cursor(Direction::Up);
                true
            }
            KeyCode::Down => {
                self.vim_editor.move_cursor(Direction::Down);
                true
            }
            _ => false,
        }
    }

    fn handle_table_view_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Char('e') | KeyCode::Enter => {
                self.view_mode = false;
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                // Move left in table
                true
            }
            KeyCode::Char('j') | KeyCode::Down => {
                // Move down in table
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                // Move up in table
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                // Move right in table
                true
            }
            _ => false,
        }
    }

    fn handle_table_edit_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Esc => {
                self.view_mode = true;
                true
            }
            KeyCode::Char(_c) => {
                // Edit cell content
                true
            }
            _ => false,
        }
    }

    fn handle_navigation_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Char('h') | KeyCode::Left => {
                // Navigate left
                true
            }
            KeyCode::Char('j') | KeyCode::Down => {
                // Navigate down
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                // Navigate up
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                // Navigate right
                true
            }
            KeyCode::Enter => {
                // Activate/expand item
                true
            }
            _ => false,
        }
    }

    fn handle_modal_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Esc => {
                // Close modal
                true
            }
            KeyCode::Enter => {
                // Confirm modal action
                true
            }
            _ => false,
        }
    }

    /// Enter edit mode for the current box
    pub fn enter_edit_mode(&mut self) -> bool {
        match self.active_box {
            Some(Box::TextInput) => {
                self.view_mode = false;
                true
            }
            Some(Box::DataTable) => {
                self.view_mode = false;
                true
            }
            Some(Box::TreeView) | Some(Box::ListView) => {
                // These boxes can't be edited, but we can enter a special mode
                false
            }
            Some(Box::Modal) => {
                // Modals are always in edit mode
                false
            }
            None => false,
        }
    }

    /// Exit edit mode for the current box
    pub fn exit_edit_mode(&mut self) -> bool {
        match self.active_box {
            Some(Box::TextInput) => {
                // Text input boxes can't exit edit mode
                false
            }
            Some(Box::DataTable) => {
                self.view_mode = true;
                true
            }
            _ => false,
        }
    }

    /// Toggle between view and edit mode
    pub fn toggle_mode(&mut self) -> bool {
        if self.view_mode {
            self.enter_edit_mode()
        } else {
            self.exit_edit_mode()
        }
    }

    /// Check if the current box supports editing
    pub fn can_edit(&self) -> bool {
        match self.active_box {
            Some(Box::TextInput) | Some(Box::DataTable) => true,
            _ => false,
        }
    }

    /// Check if the current box supports view mode
    pub fn has_view_mode(&self) -> bool {
        match self.active_box {
            Some(Box::DataTable) | Some(Box::TreeView) | Some(Box::ListView) => true,
            _ => false,
        }
    }
}