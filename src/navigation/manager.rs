use crate::navigation::types::{
    Pane, Box, Direction, NavigationConfig, NavigationState, EditingMode, VimMode, 
    KeyCombination, KeyMapping, NavigationAction
};
use crate::navigation::box_manager::BoxManager;
use crossterm::event::{KeyCode, KeyModifiers};

/// Main navigation manager that handles pane and box navigation
pub struct NavigationManager {
    /// Current navigation state
    state: NavigationState,
    /// Box manager for handling boxes within panes
    box_manager: BoxManager,
    /// Navigation configuration
    config: NavigationConfig,
    /// Available panes in order
    pane_order: Vec<Pane>,
}

impl NavigationManager {
    pub fn new(config: NavigationConfig) -> Self {
        let pane_order = vec![
            Pane::Connections,
            Pane::QueryInput,
            Pane::Results,
            Pane::SchemaExplorer,
            Pane::CommandLine,
        ];

        Self {
            state: NavigationState::default(),
            box_manager: BoxManager::new(),
            config,
            pane_order,
        }
    }

    pub fn state(&self) -> &NavigationState {
        &self.state
    }

    pub fn box_manager(&self) -> &BoxManager {
        &self.box_manager
    }

    pub fn box_manager_mut(&mut self) -> &mut BoxManager {
        &mut self.box_manager
    }

    /// Handle a navigation action
    pub fn handle_action(&mut self, action: NavigationAction) -> bool {
        match action {
            // Pane navigation
            NavigationAction::FocusConnections => {
                self.focus_pane(Pane::Connections)
            }
            NavigationAction::FocusQueryInput => {
                self.focus_pane(Pane::QueryInput)
            }
            NavigationAction::FocusResults => {
                self.focus_pane(Pane::Results)
            }
            NavigationAction::FocusSchemaExplorer => {
                self.focus_pane(Pane::SchemaExplorer)
            }
            NavigationAction::FocusCommandLine => {
                self.focus_pane(Pane::CommandLine)
            }
            NavigationAction::NextPane => {
                self.next_pane()
            }
            NavigationAction::PreviousPane => {
                self.previous_pane()
            }
            
            // Box navigation
            NavigationAction::FocusTextInput => {
                self.focus_box(Box::TextInput)
            }
            NavigationAction::FocusDataTable => {
                self.focus_box(Box::DataTable)
            }
            NavigationAction::FocusTreeView => {
                self.focus_box(Box::TreeView)
            }
            NavigationAction::FocusListView => {
                self.focus_box(Box::ListView)
            }
            NavigationAction::FocusModal => {
                self.focus_box(Box::Modal)
            }
            NavigationAction::NextBox => {
                self.next_box()
            }
            NavigationAction::PreviousBox => {
                self.previous_box()
            }
            
            // Movement
            NavigationAction::MoveLeft => {
                self.handle_directional_move(Direction::Left)
            }
            NavigationAction::MoveRight => {
                self.handle_directional_move(Direction::Right)
            }
            NavigationAction::MoveUp => {
                self.handle_directional_move(Direction::Up)
            }
            NavigationAction::MoveDown => {
                self.handle_directional_move(Direction::Down)
            }
            NavigationAction::MoveToStart => {
                // Handle move to start of line/field
                self.box_manager.vim_editor_mut().move_to_line_start();
                true
            }
            NavigationAction::MoveToEnd => {
                // Handle move to end of line/field
                self.box_manager.vim_editor_mut().move_to_line_end();
                true
            }
            NavigationAction::MoveToNextWord => {
                // Handle move to next word
                self.box_manager.vim_editor_mut().move_to_next_word();
                true
            }
            NavigationAction::MoveToPreviousWord => {
                // Handle move to previous word
                self.box_manager.vim_editor_mut().move_to_previous_word();
                true
            }
            
            // Mode switching
            NavigationAction::EnterInsertMode => {
                self.box_manager.vim_editor_mut().mode = VimMode::Insert;
                true
            }
            NavigationAction::EnterVisualMode => {
                self.box_manager.vim_editor_mut().mode = VimMode::Visual;
                true
            }
            NavigationAction::EnterCommandMode => {
                self.box_manager.vim_editor_mut().mode = VimMode::Command;
                true
            }
            NavigationAction::EnterNormalMode => {
                self.box_manager.vim_editor_mut().mode = VimMode::Normal;
                true
            }
            NavigationAction::EnterEditMode => {
                self.enter_edit_mode()
            }
            NavigationAction::ExitEditMode => {
                self.exit_edit_mode()
            }
            NavigationAction::ToggleViewEditMode => {
                self.toggle_mode()
            }
            
            // Text editing
            NavigationAction::InsertChar => {
                // This would be handled by the box manager when a character is typed
                false
            }
            NavigationAction::DeleteChar => {
                self.box_manager.vim_editor_mut().delete_char_at_cursor();
                true
            }
            NavigationAction::DeleteCharBefore => {
                self.box_manager.vim_editor_mut().delete_char_before_cursor();
                true
            }
            NavigationAction::DeleteLine => {
                self.box_manager.vim_editor_mut().delete_line();
                true
            }
            NavigationAction::ReplaceChar => {
                // This would be handled by the box manager when in replace mode
                false
            }
            NavigationAction::Undo => {
                // TODO: Implement undo functionality
                false
            }
            NavigationAction::Redo => {
                // TODO: Implement redo functionality
                false
            }
            
            // Special actions
            NavigationAction::Quit => {
                // This would be handled by the main application
                false
            }
            NavigationAction::Confirm => {
                // This would be handled by the specific context
                false
            }
            NavigationAction::Cancel => {
                // This would be handled by the specific context
                false
            }
            NavigationAction::Search => {
                // This would be handled by the main application
                false
            }
            NavigationAction::Copy => {
                // This would be handled by the main application
                false
            }
            NavigationAction::Paste => {
                // This would be handled by the main application
                false
            }
            NavigationAction::Cut => {
                // This would be handled by the main application
                false
            }
        }
    }

    /// Handle a key event and return whether it was consumed
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        // Check if this key combination maps to a navigation action
        if let Some(action) = self.config.key_mapping.get_action(key, modifiers) {
            return self.handle_action(action);
        }

        // Delegate to box manager for box-specific handling
        self.box_manager.handle_key(key, modifiers)
    }


    fn handle_directional_move(&mut self, direction: Direction) -> bool {
        // If we have an active box, let it handle the movement
        if self.box_manager.active_box().is_some() {
            return self.box_manager.handle_key(
                match direction {
                    Direction::Left => KeyCode::Left,
                    Direction::Right => KeyCode::Right,
                    Direction::Up => KeyCode::Up,
                    Direction::Down => KeyCode::Down,
                },
                KeyModifiers::empty(),
            );
        }

        // Otherwise, move between panes
        match direction {
            Direction::Left => self.previous_pane(),
            Direction::Right => self.next_pane(),
            Direction::Up | Direction::Down => {
                // For up/down, we might want to move between boxes within the pane
                match direction {
                    Direction::Up => self.previous_box(),
                    Direction::Down => self.next_box(),
                    _ => false,
                }
            }
        }
    }

    fn focus_pane(&mut self, pane: Pane) -> bool {
        if self.state.active_pane != pane {
            self.state.active_pane = pane;
            self.state.active_box = None;
            
            // Set the first available box as active
            let available_boxes = self.box_manager.get_available_boxes(pane);
            if let Some(&first_box) = available_boxes.first() {
                self.focus_box(first_box);
            }
            
            true
        } else {
            false
        }
    }

    fn focus_box(&mut self, box_type: Box) -> bool {
        let available_boxes = self.box_manager.get_available_boxes(self.state.active_pane);
        if available_boxes.contains(&box_type) {
            self.box_manager.set_active_box(Some(box_type));
            self.state.active_box = Some(box_type);
            true
        } else {
            false
        }
    }

    fn next_pane(&mut self) -> bool {
        let current_index = self.pane_order
            .iter()
            .position(|&p| p == self.state.active_pane)
            .unwrap_or(0);
        
        let next_index = (current_index + 1) % self.pane_order.len();
        let next_pane = self.pane_order[next_index];
        self.focus_pane(next_pane)
    }

    fn previous_pane(&mut self) -> bool {
        let current_index = self.pane_order
            .iter()
            .position(|&p| p == self.state.active_pane)
            .unwrap_or(0);
        
        let prev_index = if current_index == 0 {
            self.pane_order.len() - 1
        } else {
            current_index - 1
        };
        
        let prev_pane = self.pane_order[prev_index];
        self.focus_pane(prev_pane)
    }

    fn next_box(&mut self) -> bool {
        if let Some(new_box) = self.box_manager.next_box(self.state.active_pane) {
            self.state.active_box = Some(new_box);
            true
        } else {
            false
        }
    }

    fn previous_box(&mut self) -> bool {
        if let Some(new_box) = self.box_manager.previous_box(self.state.active_pane) {
            self.state.active_box = Some(new_box);
            true
        } else {
            false
        }
    }

    fn enter_edit_mode(&mut self) -> bool {
        if self.box_manager.enter_edit_mode() {
            self.state.view_mode = false;
            true
        } else {
            false
        }
    }

    fn exit_edit_mode(&mut self) -> bool {
        if self.box_manager.exit_edit_mode() {
            self.state.view_mode = true;
            true
        } else {
            false
        }
    }

    fn toggle_mode(&mut self) -> bool {
        if self.box_manager.toggle_mode() {
            self.state.view_mode = !self.state.view_mode;
            true
        } else {
            false
        }
    }

    /// Get the current mode indicator text
    pub fn get_mode_indicator(&self) -> String {
        if self.state.editing_mode == EditingMode::Vim {
            match self.box_manager.vim_editor().mode() {
                VimMode::Normal => "NORMAL".to_string(),
                VimMode::Insert => "INSERT".to_string(),
                VimMode::Visual => "VISUAL".to_string(),
                VimMode::Command => "COMMAND".to_string(),
            }
        } else {
            if self.box_manager.can_edit() {
                if self.state.view_mode {
                    "VIEW".to_string()
                } else {
                    "EDIT".to_string()
                }
            } else {
                "NAV".to_string()
            }
        }
    }

    /// Get the current pane and box information
    pub fn get_navigation_info(&self) -> String {
        let pane_name = match self.state.active_pane {
            Pane::Connections => "Connections",
            Pane::QueryInput => "Query",
            Pane::Results => "Results",
            Pane::SchemaExplorer => "Schema",
            Pane::CommandLine => "Command",
        };

        let box_name = match self.state.active_box {
            Some(Box::TextInput) => " (Text)",
            Some(Box::DataTable) => " (Table)",
            Some(Box::TreeView) => " (Tree)",
            Some(Box::ListView) => " (List)",
            Some(Box::Modal) => " (Modal)",
            None => "",
        };

        format!("{}{}", pane_name, box_name)
    }
}