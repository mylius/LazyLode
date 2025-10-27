use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{layout::Rect, Frame};

use crate::app::App;
use crate::ui::modal_manager::{Modal, ModalResult};

/// Modal for command input
#[derive(Debug)]
pub struct CommandModal {
    // TODO: Move command input state here
}

impl CommandModal {
    pub fn new() -> Self {
        Self {}
    }
}

impl Modal for CommandModal {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        // TODO: Implement command modal rendering
        use ratatui::style::Style;
        use ratatui::widgets::{Block, Borders, Clear};
        
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title("Command Modal (TODO)")
                .borders(Borders::ALL)
                .style(Style::default().fg(app.config.theme.text_color())),
            area,
        );
    }

    fn handle_input(&mut self, key: KeyCode, _modifiers: KeyModifiers, _nav_action: Option<crate::navigation::types::NavigationAction>) -> ModalResult {
        // Handle common modal keys
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                return ModalResult::Closed;
            }
            _ => {}
        }

        // TODO: Implement command modal input handling
        ModalResult::Continue
    }

    fn get_title(&self) -> &str {
        "Command"
    }

    fn get_size(&self) -> (u16, u16) {
        (60, 3)
    }
}
