use crate::navigation::types::{VimMode, Direction};
use crossterm::event::{KeyCode, KeyModifiers};

/// Vim-style text editor for handling text input with vim keybindings
pub struct VimEditor {
    /// Current vim mode
    pub mode: VimMode,
    /// Text content
    content: String,
    /// Cursor position (row, column)
    cursor_position: (usize, usize),
    /// Whether we're in replace mode
    replace_mode: bool,
    /// Last key pressed (for double-key commands like 'dd')
    last_key: Option<char>,
}

impl VimEditor {
    pub fn new() -> Self {
        Self {
            mode: VimMode::Normal,
            content: String::new(),
            cursor_position: (0, 0),
            replace_mode: false,
            last_key: None,
        }
    }

    pub fn with_content(content: String) -> Self {
        Self {
            content,
            ..Self::new()
        }
    }

    pub fn mode(&self) -> VimMode {
        self.mode
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        self.cursor_position
    }

    pub fn set_cursor_position(&mut self, pos: (usize, usize)) {
        self.cursor_position = pos;
    }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor_position = (0, 0);
    }

    /// Handle a key event and return whether it was consumed
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        if modifiers != KeyModifiers::empty() {
            return false; // Don't handle modified keys for now
        }

        match self.mode {
            VimMode::Normal => self.handle_normal_mode(key),
            VimMode::Insert => self.handle_insert_mode(key),
            VimMode::Visual => self.handle_visual_mode(key),
            VimMode::Command => self.handle_command_mode(key),
        }
    }

    fn handle_normal_mode(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('i') => {
                self.mode = VimMode::Insert;
                true
            }
            KeyCode::Char('a') => {
                self.mode = VimMode::Insert;
                self.move_cursor(Direction::Right);
                true
            }
            KeyCode::Char('o') => {
                self.mode = VimMode::Insert;
                self.insert_newline();
                true
            }
            KeyCode::Char('O') => {
                self.mode = VimMode::Insert;
                self.insert_newline_above();
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.move_cursor(Direction::Left);
                true
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_cursor(Direction::Down);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_cursor(Direction::Up);
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.move_cursor(Direction::Right);
                true
            }
            KeyCode::Char('w') => {
                self.move_to_next_word();
                true
            }
            KeyCode::Char('b') => {
                self.move_to_previous_word();
                true
            }
            KeyCode::Char('0') => {
                self.move_to_line_start();
                true
            }
            KeyCode::Char('$') => {
                self.move_to_line_end();
                true
            }
            KeyCode::Char('d') => {
                if self.last_key == Some('d') {
                    self.delete_line();
                    self.last_key = None;
                } else {
                    self.last_key = Some('d');
                }
                true
            }
            KeyCode::Char('x') => {
                self.delete_char_at_cursor();
                true
            }
            KeyCode::Char('r') => {
                self.replace_mode = true;
                true
            }
            KeyCode::Char(':') => {
                self.mode = VimMode::Command;
                true
            }
            KeyCode::Char('v') => {
                self.mode = VimMode::Visual;
                true
            }
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                self.replace_mode = false;
                self.last_key = None;
                true
            }
            _ => false,
        }
    }

    fn handle_insert_mode(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                true
            }
            KeyCode::Char(c) => {
                if self.replace_mode {
                    self.replace_char_at_cursor(c);
                    self.replace_mode = false;
                } else {
                    self.insert_char_at_cursor(c);
                }
                true
            }
            KeyCode::Backspace => {
                self.delete_char_before_cursor();
                true
            }
            KeyCode::Enter => {
                self.insert_newline();
                true
            }
            KeyCode::Left => {
                self.move_cursor(Direction::Left);
                true
            }
            KeyCode::Right => {
                self.move_cursor(Direction::Right);
                true
            }
            KeyCode::Up => {
                self.move_cursor(Direction::Up);
                true
            }
            KeyCode::Down => {
                self.move_cursor(Direction::Down);
                true
            }
            _ => false,
        }
    }

    fn handle_visual_mode(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.move_cursor(Direction::Left);
                true
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_cursor(Direction::Down);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_cursor(Direction::Up);
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.move_cursor(Direction::Right);
                true
            }
            _ => false,
        }
    }

    fn handle_command_mode(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                true
            }
            KeyCode::Enter => {
                // Execute command (would be handled by parent)
                self.mode = VimMode::Normal;
                true
            }
            _ => false,
        }
    }

    pub fn move_cursor(&mut self, direction: Direction) {
        let (row, col) = self.cursor_position;
        let lines: Vec<&str> = self.content.lines().collect();
        
        if lines.is_empty() {
            return;
        }

        match direction {
            Direction::Left => {
                if col > 0 {
                    self.cursor_position = (row, col - 1);
                }
            }
            Direction::Right => {
                let line_len = lines.get(row).map(|l| l.len()).unwrap_or(0);
                if col < line_len {
                    self.cursor_position = (row, col + 1);
                }
            }
            Direction::Up => {
                if row > 0 {
                    let new_col = col.min(lines[row - 1].len());
                    self.cursor_position = (row - 1, new_col);
                }
            }
            Direction::Down => {
                if row + 1 < lines.len() {
                    let new_col = col.min(lines[row + 1].len());
                    self.cursor_position = (row + 1, new_col);
                }
            }
        }
    }

    pub fn move_to_next_word(&mut self) {
        // Simplified word movement - move to next space or end of line
        let (row, col) = self.cursor_position;
        let lines: Vec<&str> = self.content.lines().collect();
        
        if let Some(line) = lines.get(row) {
            let remaining = &line[col..];
            if let Some(pos) = remaining.find(' ') {
                self.cursor_position = (row, col + pos + 1);
            } else {
                self.cursor_position = (row, line.len());
            }
        }
    }

    pub fn move_to_previous_word(&mut self) {
        // Simplified word movement - move to previous space or start of line
        let (row, col) = self.cursor_position;
        let lines: Vec<&str> = self.content.lines().collect();
        
        if let Some(line) = lines.get(row) {
            let before = &line[..col];
            if let Some(pos) = before.rfind(' ') {
                self.cursor_position = (row, pos + 1);
            } else {
                self.cursor_position = (row, 0);
            }
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_position = (self.cursor_position.0, 0);
    }

    pub fn move_to_line_end(&mut self) {
        let (row, _) = self.cursor_position;
        let lines: Vec<&str> = self.content.lines().collect();
        
        if let Some(line) = lines.get(row) {
            self.cursor_position = (row, line.len());
        }
    }

    pub fn insert_char_at_cursor(&mut self, c: char) {
        let (row, col) = self.cursor_position;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        if lines.is_empty() {
            lines.push(String::new());
        }
        
        if let Some(line) = lines.get_mut(row) {
            line.insert(col, c);
            self.cursor_position = (row, col + 1);
        }
        
        self.content = lines.join("\n");
    }

    fn replace_char_at_cursor(&mut self, c: char) {
        let (row, col) = self.cursor_position;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        if let Some(line) = lines.get_mut(row) {
            if col < line.len() {
                line.replace_range(col..col + 1, &c.to_string());
                self.cursor_position = (row, col + 1);
            }
        }
        
        self.content = lines.join("\n");
    }

    pub fn delete_char_at_cursor(&mut self) {
        let (row, col) = self.cursor_position;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        if let Some(line) = lines.get_mut(row) {
            if col < line.len() {
                line.remove(col);
            }
        }
        
        self.content = lines.join("\n");
    }

    pub fn delete_char_before_cursor(&mut self) {
        let (row, col) = self.cursor_position;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        if let Some(line) = lines.get_mut(row) {
            if col > 0 {
                line.remove(col - 1);
                self.cursor_position = (row, col - 1);
            }
        }
        
        self.content = lines.join("\n");
    }

    pub fn delete_line(&mut self) {
        let row = self.cursor_position.0;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        if row < lines.len() {
            lines.remove(row);
            if lines.is_empty() {
                lines.push(String::new());
            }
            self.cursor_position = (row.min(lines.len() - 1), 0);
        }
        
        self.content = lines.join("\n");
    }

    pub fn insert_newline(&mut self) {
        let (row, col) = self.cursor_position;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        if let Some(line) = lines.get_mut(row) {
            let before = line[..col].to_string();
            let after = line[col..].to_string();
            *line = before;
            lines.insert(row + 1, after);
        }
        
        self.cursor_position = (row + 1, 0);
        self.content = lines.join("\n");
    }

    fn insert_newline_above(&mut self) {
        let (row, col) = self.cursor_position;
        let mut lines: Vec<String> = self.content.lines().map(|s| s.to_string()).collect();
        
        lines.insert(row, String::new());
        self.cursor_position = (row, 0);
        self.content = lines.join("\n");
    }
}