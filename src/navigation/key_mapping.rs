use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Represents a key combination (key + modifiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombination {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombination {
    pub fn new(key: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn simple(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::empty(),
        }
    }

    pub fn with_ctrl(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::CONTROL,
        }
    }

    pub fn with_alt(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::ALT,
        }
    }

    pub fn with_shift(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::SHIFT,
        }
    }
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }
        
        let key_str = match self.key {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "BackTab".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Insert => "Insert".to_string(),
            KeyCode::F(n) => format!("F{}", n),
            KeyCode::Null => "Null".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            _ => format!("{:?}", self.key),
        };
        
        parts.push(&key_str);
        write!(f, "{}", parts.join("+"))
    }
}

/// All possible navigation actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NavigationAction {
    // Pane navigation
    FocusConnections,
    FocusQueryInput,
    FocusResults,
    FocusSchemaExplorer,
    FocusCommandLine,
    NextPane,
    PreviousPane,
    
    // Box navigation
    FocusTextInput,
    FocusDataTable,
    FocusTreeView,
    FocusListView,
    FocusModal,
    NextBox,
    PreviousBox,
    
    // Movement
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveToStart,
    MoveToEnd,
    MoveToNextWord,
    MoveToPreviousWord,
    
    // Editing modes
    EnterInsertMode,
    EnterVisualMode,
    EnterCommandMode,
    EnterNormalMode,
    EnterEditMode,
    ExitEditMode,
    ToggleViewEditMode,
    
    // Text editing
    InsertChar,
    DeleteChar,
    DeleteCharBefore,
    DeleteLine,
    ReplaceChar,
    Undo,
    Redo,
    
    // Special actions
    Quit,
    Confirm,
    Cancel,
    Search,
    Copy,
    Paste,
    Cut,
}

/// Key mapping configuration that maps key combinations to actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    /// Maps key combinations to navigation actions
    pub mappings: HashMap<KeyCombination, NavigationAction>,
}

impl KeyMapping {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Add a key mapping
    pub fn add_mapping(&mut self, key_combo: KeyCombination, action: NavigationAction) {
        self.mappings.insert(key_combo, action);
    }

    /// Remove a key mapping
    pub fn remove_mapping(&mut self, key_combo: KeyCombination) {
        self.mappings.remove(&key_combo);
    }

    /// Get action for a key combination
    pub fn get_action(&self, key: KeyCode, modifiers: KeyModifiers) -> Option<NavigationAction> {
        let key_combo = KeyCombination::new(key, modifiers);
        self.mappings.get(&key_combo).copied()
    }

    /// Get all key combinations for a specific action
    pub fn get_keys_for_action(&self, action: NavigationAction) -> Vec<KeyCombination> {
        self.mappings
            .iter()
            .filter(|(_, &a)| a == action)
            .map(|(&k, _)| k)
            .collect()
    }

    /// Check if a key combination is mapped
    pub fn is_mapped(&self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        let key_combo = KeyCombination::new(key, modifiers);
        self.mappings.contains_key(&key_combo)
    }

    /// Get all mappings as a vector of (key_combo, action) pairs
    pub fn get_all_mappings(&self) -> Vec<(KeyCombination, NavigationAction)> {
        self.mappings.iter().map(|(&k, &v)| (k, v)).collect()
    }
}

impl Default for KeyMapping {
    fn default() -> Self {
        let mut mapping = Self::new();
        
        // Default key mappings
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('q')), NavigationAction::Quit);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Esc), NavigationAction::Cancel);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('/')), NavigationAction::Search);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Enter), NavigationAction::Confirm);
        
        // Pane navigation
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('c')), NavigationAction::FocusConnections);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('q')), NavigationAction::FocusQueryInput);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('r')), NavigationAction::FocusResults);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('s')), NavigationAction::FocusSchemaExplorer);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char(':')), NavigationAction::FocusCommandLine);
        
        // Box navigation
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('t')), NavigationAction::FocusTextInput);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('d')), NavigationAction::FocusDataTable);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('v')), NavigationAction::FocusTreeView);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('l')), NavigationAction::FocusListView);
        
        // Movement
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('h')), NavigationAction::MoveLeft);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('j')), NavigationAction::MoveDown);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('k')), NavigationAction::MoveUp);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('l')), NavigationAction::MoveRight);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Left), NavigationAction::MoveLeft);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Right), NavigationAction::MoveRight);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Up), NavigationAction::MoveUp);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Down), NavigationAction::MoveDown);
        
        // Vim-style editing
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('i')), NavigationAction::EnterInsertMode);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('a')), NavigationAction::EnterInsertMode);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('v')), NavigationAction::EnterVisualMode);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char(':')), NavigationAction::EnterCommandMode);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Esc), NavigationAction::EnterNormalMode);
        
        // Edit mode switching
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('e')), NavigationAction::EnterEditMode);
        mapping.add_mapping(KeyCombination::with_ctrl(KeyCode::Char('v')), NavigationAction::ToggleViewEditMode);
        
        // Text editing
        mapping.add_mapping(KeyCombination::simple(KeyCode::Backspace), NavigationAction::DeleteCharBefore);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Delete), NavigationAction::DeleteChar);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('x')), NavigationAction::DeleteChar);
        mapping.add_mapping(KeyCombination::simple(KeyCode::Char('r')), NavigationAction::ReplaceChar);
        
        // Copy/paste
        mapping.add_mapping(KeyCombination::with_ctrl(KeyCode::Char('c')), NavigationAction::Copy);
        mapping.add_mapping(KeyCombination::with_ctrl(KeyCode::Char('v')), NavigationAction::Paste);
        mapping.add_mapping(KeyCombination::with_ctrl(KeyCode::Char('x')), NavigationAction::Cut);
        
        mapping
    }
}

/// Helper trait for creating key combinations more easily
pub trait KeyComboExt {
    fn to_combo(self) -> KeyCombination;
    fn with_ctrl(self) -> KeyCombination;
    fn with_alt(self) -> KeyCombination;
    fn with_shift(self) -> KeyCombination;
}

impl KeyComboExt for KeyCode {
    fn to_combo(self) -> KeyCombination {
        KeyCombination::simple(self)
    }
    
    fn with_ctrl(self) -> KeyCombination {
        KeyCombination::with_ctrl(self)
    }
    
    fn with_alt(self) -> KeyCombination {
        KeyCombination::with_alt(self)
    }
    
    fn with_shift(self) -> KeyCombination {
        KeyCombination::with_shift(self)
    }
}