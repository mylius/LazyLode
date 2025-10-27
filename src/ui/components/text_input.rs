use crate::navigation::types::VimMode;
use crate::navigation::vim_editor::VimEditor;
use crossterm::event::{KeyCode, KeyModifiers};

/// Thin wrapper around VimEditor for single-line inputs
/// Just delegates to VimEditor's methods
#[derive(Debug, Clone)]
pub struct TextInput {
    vim_editor: VimEditor,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            vim_editor: VimEditor::new(),
        }
    }

    pub fn content(&self) -> &str {
        self.vim_editor.content()
    }

    pub fn mode(&self) -> VimMode {
        self.vim_editor.mode()
    }

    pub fn cursor_position(&self) -> usize {
        self.vim_editor.cursor_position().1
    }

    pub fn set_mode(&mut self, mode: VimMode) {
        self.vim_editor.mode = mode;
    }

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        if modifiers.is_empty() && self.mode() == VimMode::Normal {
            match key {
                KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('k') | KeyCode::Up => {
                    return false;
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.vim_editor.paste();
                    return true;
                }
                _ => {}
            }
        }
        self.vim_editor.handle_key(key, modifiers)
    }

    pub fn insert_char_at_cursor(&mut self, c: char) {
        self.vim_editor.insert_char_at_cursor(c);
    }

    pub fn display_text_with_cursor(&self) -> String {
        let content = self.vim_editor.content();
        let cursor = self.cursor_position().min(content.len());

        if self.mode() == VimMode::Insert {
            if content.is_empty() {
                "|".to_string()
            } else if cursor == content.len() {
                format!("{}|", content)
            } else {
                let (before, after) = content.split_at(cursor);
                format!("{}|{}", before, after)
            }
        } else {
            content.to_string()
        }
    }
}
