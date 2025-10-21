use crate::app::App;
use crate::ui::types::Direction;
use anyhow::Result;

pub struct CommandBuffer {
    // The buffer of characters entered
    buffer: Vec<char>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    // Add a character to the buffer
    pub fn push(&mut self, c: char) {
        self.buffer.push(c);
    }

    // Get the current buffer as a string
    pub fn as_str(&self) -> String {
        self.buffer.iter().collect()
    }

    // Check if the buffer matches a specific command
    pub fn matches(&self, command: &str) -> bool {
        self.as_str() == command
    }

    // Check if the buffer starts with a specific prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.as_str().starts_with(prefix)
    }

    // Check if the buffer contains a digit prefix
    pub fn get_numeric_prefix(&self) -> Option<usize> {
        let s = self.as_str();
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();

        if !digits.is_empty() {
            digits.parse::<usize>().ok()
        } else {
            None
        }
    }

    // Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    // Remove the last character from the buffer
    pub fn pop(&mut self) {
        self.buffer.pop();
    }

    // Get the current length of the buffer
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    // Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

pub struct CommandProcessor;

impl CommandProcessor {
    // Process the current command buffer and execute the appropriate action
    pub fn process_command(app: &mut App) -> Result<bool> {
        let command = app.command_buffer.as_str();

        // If empty, nothing to process
        if command.is_empty() {
            return Ok(false);
        }

        // Extract numeric prefix if any
        let count = app.command_buffer.get_numeric_prefix().unwrap_or(1);

        // Check for movement commands (like "3j")
        if let Some(last_char) = command.chars().last() {
            match last_char {
                'j' => {
                    // Move down count times
                    for _ in 0..count {
                        app.move_cursor_in_results(Direction::Down);
                    }
                    app.command_buffer.clear();
                    return Ok(true);
                }
                'k' => {
                    // Move up count times
                    for _ in 0..count {
                        app.move_cursor_in_results(Direction::Up);
                    }
                    app.command_buffer.clear();
                    return Ok(true);
                }
                'h' => {
                    // Move left count times
                    for _ in 0..count {
                        app.move_cursor_in_results(Direction::Left);
                    }
                    app.command_buffer.clear();
                    return Ok(true);
                }
                'l' => {
                    // Move right count times
                    for _ in 0..count {
                        app.move_cursor_in_results(Direction::Right);
                    }
                    app.command_buffer.clear();
                    return Ok(true);
                }
                // Add other single-character commands here
                _ => {}
            }
        }

        // Check for specific commands
        match command.as_str() {
            "yy" => {
                app.copy_row()?;
                app.command_buffer.clear();
                return Ok(true);
            }
            "y" => {
                app.copy_cell()?;
                return Ok(true);
            }
            ":themes" => {
                app.list_themes()?;
                app.command_buffer.clear();
                return Ok(true);
            }
            _ => {}
        }

        // Check for theme switching commands
        if command.starts_with(":theme ") {
            let theme_name = command.strip_prefix(":theme ").unwrap_or("");
            if !theme_name.is_empty() {
                app.switch_theme(theme_name)?;
                app.command_buffer.clear();
                return Ok(true);
            }
        }

        // If command starts with 'y' but wasn't handled above,
        // treat it as a completed cell copy
        if command.starts_with('y') && command.len() > 1 {
            app.copy_cell()?;
            app.command_buffer.clear();
            return Ok(true);
        }

        // Command not recognized
        return Ok(false);
    }
}
