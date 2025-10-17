use crate::input::{Action, KeyConfig, PaneModifier};
use crossterm::event::{KeyCode, KeyModifiers};

fn main() {
    let keymap = KeyConfig::default();
    
    // Test colon key
    let colon_action = keymap.get_action(KeyCode::Char(':'), KeyModifiers::empty());
    println!("Colon action: {:?}", colon_action);
    
    // Test other keys
    let h_action = keymap.get_action(KeyCode::Char('h'), KeyModifiers::empty());
    println!("H action: {:?}", h_action);
    
    let q_action = keymap.get_action(KeyCode::Char('q'), KeyModifiers::empty());
    println!("Q action: {:?}", q_action);
}