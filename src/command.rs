use crate::app::App;
use crate::ui::types::Direction;
use anyhow::Result;

/// Simple fuzzy matching function
/// Returns true if the pattern can be found in the text with characters in order
fn fuzzy_match(pattern: &str, text: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    if pattern_lower.is_empty() {
        return true;
    }

    let mut pattern_chars = pattern_lower.chars().peekable();
    let text_chars = text_lower.chars();

    for text_char in text_chars {
        if let Some(&pattern_char) = pattern_chars.peek() {
            if text_char == pattern_char {
                pattern_chars.next();
                if pattern_chars.peek().is_none() {
                    return true;
                }
            }
        }
    }

    false
}

/// Represents a command with its name and action
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub action: fn(&mut App) -> Result<()>,
}

impl Command {
    pub fn new(name: &str, description: &str, action: fn(&mut App) -> Result<()>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            action,
        }
    }

    pub fn execute(&self, app: &mut App) -> Result<()> {
        (self.action)(app)
    }
}

/// Registry of all available commands
pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: Vec::new(),
        };
        registry.register_default_commands();
        registry
    }

    fn register_default_commands(&mut self) {
        // Theme commands
        self.register(Command::new("listThemes", "List available themes", |app| {
            app.list_themes()?;
            Ok(())
        }));

        self.register(Command::new(
            "switchTheme",
            "Switch theme (use 'switchTheme <name>')",
            |_app| {
                // This is handled specially for theme switching with parameters
                Ok(())
            },
        ));

        // Quit commands
        self.register(Command::new("quit", "Quit the application", |app| {
            app.quit();
            app.set_status_message("Quitting...".to_string());
            Ok(())
        }));

        self.register(Command::new("q", "Quit the application", |app| {
            app.quit();
            app.set_status_message("Quitting...".to_string());
            Ok(())
        }));

        // Connection commands
        self.register(Command::new(
            "addConnection",
            "Add a new database connection",
            |app| {
                app.toggle_connection_modal();
                app.set_status_message("Connection modal opened".to_string());
                Ok(())
            },
        ));
    }

    pub fn register(&mut self, command: Command) {
        self.commands.push(command);
    }

    pub fn get_suggestions(&self, input: &str) -> Vec<String> {
        let input_lower = input.to_lowercase();
        let mut suggestions = Vec::new();

        for command in &self.commands {
            if fuzzy_match(&input_lower, &command.name) {
                suggestions.push(command.name.clone());
            }
        }

        // Handle theme suggestions with parameters
        if input_lower.starts_with("theme ") || input_lower.starts_with("switchtheme ") {
            let (command_prefix, theme_query) = if input_lower.starts_with("theme ") {
                ("theme", input_lower.strip_prefix("theme ").unwrap_or(""))
            } else {
                (
                    "switchTheme",
                    input_lower.strip_prefix("switchtheme ").unwrap_or(""),
                )
            };
            if let Ok(themes) = crate::config::Config::list_themes() {
                for theme in themes {
                    if theme_query.is_empty() || fuzzy_match(theme_query, &theme) {
                        suggestions.push(format!("{} {}", command_prefix, theme));
                    }
                }
            }
        }

        suggestions.sort();
        suggestions
    }

    pub fn execute_command(&self, command_name: &str, app: &mut App) -> Result<bool> {
        let command_name_lower = command_name.to_lowercase();

        // Handle theme switching with parameters
        if command_name_lower.starts_with("theme ")
            || command_name_lower.starts_with("switchtheme ")
        {
            let theme_name = if command_name_lower.starts_with("theme ") {
                command_name_lower.strip_prefix("theme ").unwrap_or("")
            } else {
                command_name_lower
                    .strip_prefix("switchtheme ")
                    .unwrap_or("")
            };
            if !theme_name.is_empty() {
                app.switch_theme(theme_name)?;
                app.set_status_message(format!("Switched to theme: {}", theme_name));
                return Ok(true);
            }
        }

        // Handle exact command matches
        for command in &self.commands {
            if command.name.to_lowercase() == command_name_lower {
                command.execute(app)?;
                return Ok(true);
            }
        }

        Ok(false)
    }
}

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

pub struct CommandProcessor {
    registry: CommandRegistry,
}

impl CommandProcessor {
    pub fn new() -> Self {
        Self {
            registry: CommandRegistry::new(),
        }
    }

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

        // Use the command registry to execute commands
        let processor = Self::new();
        match processor.registry.execute_command(&command, app) {
            Ok(true) => {
                app.command_buffer.clear();
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn get_suggestions(input: &str) -> Vec<String> {
        let processor = Self::new();
        processor.registry.get_suggestions(input)
    }
}
