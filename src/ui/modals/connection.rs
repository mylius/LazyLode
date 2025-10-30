use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::ui::components::{FieldNavigator, TextInput};
use crate::ui::modal_manager::{Modal, ModalResult};

/// Modal for managing database connections
#[derive(Debug)]
pub struct ConnectionModal {
    // Modal owns its state
    name: String,
    db_type: crate::database::DatabaseType,
    host: String,
    port: String,
    username: String,
    password: String,
    database: String,
    field_navigator: FieldNavigator,
    text_inputs: Vec<TextInput>,
}

impl ConnectionModal {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            db_type: crate::database::DatabaseType::Postgres,
            host: String::new(),
            port: "5432".to_string(),
            username: String::new(),
            password: String::new(),
            database: String::new(),
            field_navigator: FieldNavigator::new(6), // name, host, port, username, password, database
            text_inputs: vec![TextInput::new(); 6],
        }
    }

    fn get_current_input(&self) -> &TextInput {
        &self.text_inputs[self.field_navigator.current_field()]
    }

    fn get_current_input_mut(&mut self) -> &mut TextInput {
        &mut self.text_inputs[self.field_navigator.current_field()]
    }

    fn sync_all_values(&mut self) {
        self.name = self.text_inputs[0].content().to_string();
        self.host = self.text_inputs[1].content().to_string();
        self.port = self.text_inputs[2].content().to_string();
        self.username = self.text_inputs[3].content().to_string();
        self.password = self.text_inputs[4].content().to_string();
        self.database = self.text_inputs[5].content().to_string();
    }
}

impl Modal for ConnectionModal {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        frame.render_widget(Clear, area);

        let block = Block::default()
            .title("New Connection")
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface1_color()),
            );

        frame.render_widget(block.clone(), area);

        let inner_area = block.inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .margin(1)
            .split(inner_area);

        let ssh_tunnel_label = "SSH Tunnel:".to_string();
        let ssh_tunnel_value = "None".to_string(); // TODO: support SSH tunnel selection

        let password_display = "*".repeat(self.text_inputs[4].content().len());

        // Build fields with cursor display for current field
        let current_field = self.field_navigator.current_field();
        let fields: Vec<(String, String)> = (0..6)
            .map(|i| {
                let label = match i {
                    0 => "Name:",
                    1 => "Host:",
                    2 => "Port:",
                    3 => "Username:",
                    4 => "Password:",
                    5 => "Database:",
                    _ => "",
                };

                let value = if i == 4 {
                    password_display.clone()
                } else if i == current_field {
                    self.text_inputs[i].display_text_with_cursor()
                } else {
                    self.text_inputs[i].content().to_string()
                };

                (label.into(), value)
            })
            .collect();

        let all_fields = vec![fields, vec![(ssh_tunnel_label, ssh_tunnel_value)]]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        for (i, (label, value)) in all_fields.iter().enumerate() {
            let style = if i == self.field_navigator.current_field() {
                Style::default().fg(app.config.theme.accent_color())
            } else {
                Style::default().fg(app.config.theme.text_color())
            };

            frame.render_widget(
                Paragraph::new(format!("{} {}", label, value)).style(style),
                chunks[i],
            );
        }
    }

    fn handle_input(
        &mut self,
        key: KeyCode,
        _modifiers: KeyModifiers,
        nav_action: Option<crate::navigation::types::NavigationAction>,
    ) -> ModalResult {
        let current = self.field_navigator.current_field();
        let current_input = &mut self.text_inputs[current];

        // Try handling all keys through VimEditor first
        if current_input.handle_key(key, _modifiers) {
            return ModalResult::Continue;
        }

        // Handle modal-specific actions
        if let Some(action) = nav_action {
            match action {
                crate::navigation::types::NavigationAction::Quit
                | crate::navigation::types::NavigationAction::Cancel => {
                    return ModalResult::Closed;
                }
                crate::navigation::types::NavigationAction::Confirm => {
                    self.sync_all_values();
                    let action = format!(
                        "create_connection:{}:{}:{}:{}:{}:{}",
                        self.name,
                        self.host,
                        self.port,
                        self.username,
                        self.password,
                        self.database
                    );
                    return ModalResult::Action(action);
                }
                _ => {}
            }

            // Allow field navigation in normal mode
            if current_input.mode() == crate::navigation::types::VimMode::Normal {
                if self.field_navigator.handle_action(action) {
                    return ModalResult::Continue;
                }
            }
        }

        ModalResult::Continue
    }

    fn get_title(&self) -> &str {
        "New Connection"
    }

    fn get_mode(&self) -> Option<crate::navigation::types::VimMode> {
        let current = self.field_navigator.current_field();
        Some(self.text_inputs[current].mode())
    }

    fn get_size(&self) -> (u16, u16) {
        (60, 50)
    }
}
