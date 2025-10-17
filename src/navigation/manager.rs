use crate::navigation::types::{
    Pane, Box, Direction, NavigationAction, NavigationConfig, NavigationState, EditingMode, VimMode
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
            NavigationAction::Move(direction) => {
                self.handle_directional_move(direction)
            }
            NavigationAction::FocusPane(pane) => {
                self.focus_pane(pane)
            }
            NavigationAction::FocusBox(box_type) => {
                self.focus_box(box_type)
            }
            NavigationAction::NextPane => {
                self.next_pane()
            }
            NavigationAction::PreviousPane => {
                self.previous_pane()
            }
            NavigationAction::NextBox => {
                self.next_box()
            }
            NavigationAction::PreviousBox => {
                self.previous_box()
            }
            NavigationAction::EnterEditMode => {
                self.enter_edit_mode()
            }
            NavigationAction::ExitEditMode => {
                self.exit_edit_mode()
            }
            NavigationAction::ToggleMode => {
                self.toggle_mode()
            }
        }
    }

    /// Handle a key event and return whether it was consumed
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        // First check for pane hotkeys
        if let KeyCode::Char(c) = key {
            if let Some(&pane) = self.config.pane_hotkeys.get(&c) {
                if modifiers == KeyModifiers::empty() {
                    self.focus_pane(pane);
                    return true;
                }
            }
        }

        // Check for box hotkeys within current pane
        if let KeyCode::Char(c) = key {
            if let Some(&box_type) = self.config.box_hotkeys.get(&c) {
                if modifiers == KeyModifiers::empty() {
                    self.focus_box(box_type);
                    return true;
                }
            }
        }

        // Handle directional navigation
        if let Some(direction) = self.key_to_direction(key) {
            if modifiers == KeyModifiers::empty() {
                return self.handle_directional_move(direction);
            }
        }

        // Delegate to box manager for box-specific handling
        self.box_manager.handle_key(key, modifiers)
    }

    fn key_to_direction(&self, key: KeyCode) -> Option<Direction> {
        match key {
            KeyCode::Char('h') | KeyCode::Left => Some(Direction::Left),
            KeyCode::Char('j') | KeyCode::Down => Some(Direction::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(Direction::Up),
            KeyCode::Char('l') | KeyCode::Right => Some(Direction::Right),
            _ => None,
        }
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