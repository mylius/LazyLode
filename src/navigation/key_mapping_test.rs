#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn test_key_combination_creation() {
        let combo = KeyCombination::simple(KeyCode::Char('a'));
        assert_eq!(combo.key, KeyCode::Char('a'));
        assert_eq!(combo.modifiers, KeyModifiers::empty());

        let combo_ctrl = KeyCombination::with_ctrl(KeyCode::Char('c'));
        assert_eq!(combo_ctrl.key, KeyCode::Char('c'));
        assert_eq!(combo_ctrl.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_key_mapping_operations() {
        let mut mapping = KeyMapping::new();
        
        // Add a mapping
        mapping.add_mapping(
            KeyCombination::simple(KeyCode::Char('q')),
            NavigationAction::Quit
        );
        
        // Test getting action
        assert_eq!(
            mapping.get_action(KeyCode::Char('q'), KeyModifiers::empty()),
            Some(NavigationAction::Quit)
        );
        
        // Test with modifiers
        mapping.add_mapping(
            KeyCombination::with_ctrl(KeyCode::Char('c')),
            NavigationAction::Copy
        );
        
        assert_eq!(
            mapping.get_action(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(NavigationAction::Copy)
        );
        
        // Test non-existent mapping
        assert_eq!(
            mapping.get_action(KeyCode::Char('x'), KeyModifiers::empty()),
            None
        );
    }

    #[test]
    fn test_default_mappings() {
        let mapping = KeyMapping::default();
        
        // Test some default mappings
        assert_eq!(
            mapping.get_action(KeyCode::Char('q'), KeyModifiers::empty()),
            Some(NavigationAction::Quit)
        );
        
        assert_eq!(
            mapping.get_action(KeyCode::Char('c'), KeyModifiers::empty()),
            Some(NavigationAction::FocusConnections)
        );
        
        assert_eq!(
            mapping.get_action(KeyCode::Char('h'), KeyModifiers::empty()),
            Some(NavigationAction::MoveLeft)
        );
    }

    #[test]
    fn test_key_combination_display() {
        let combo = KeyCombination::simple(KeyCode::Char('a'));
        assert_eq!(format!("{}", combo), "a");
        
        let combo_ctrl = KeyCombination::with_ctrl(KeyCode::Char('c'));
        assert_eq!(format!("{}", combo_ctrl), "Ctrl+c");
        
        let combo_alt_shift = KeyCombination {
            key: KeyCode::Char('x'),
            modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
        };
        assert_eq!(format!("{}", combo_alt_shift), "Alt+Shift+x");
    }

    #[test]
    fn test_get_keys_for_action() {
        let mut mapping = KeyMapping::new();
        
        // Add multiple mappings for the same action
        mapping.add_mapping(
            KeyCombination::simple(KeyCode::Char('q')),
            NavigationAction::Quit
        );
        mapping.add_mapping(
            KeyCombination::with_ctrl(KeyCode::Char('q')),
            NavigationAction::Quit
        );
        
        let keys = mapping.get_keys_for_action(NavigationAction::Quit);
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&KeyCombination::simple(KeyCode::Char('q'))));
        assert!(keys.contains(&KeyCombination::with_ctrl(KeyCode::Char('q'))));
    }
}