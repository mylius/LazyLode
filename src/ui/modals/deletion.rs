use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{layout::Rect, Frame};

use crate::app::App;
use crate::ui::modal_manager::{Modal, ModalResult};

/// Modal for confirming row deletions
#[derive(Debug)]
pub struct DeletionModal {
    // TODO: Move deletion preview state here
}

impl DeletionModal {
    pub fn new() -> Self {
        Self {}
    }
}

impl Modal for DeletionModal {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        // TODO: Implement deletion modal rendering
        use ratatui::style::Style;
        use ratatui::widgets::{Block, Borders, Clear};

        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title("Deletion Modal (TODO)")
                .borders(Borders::ALL)
                .style(Style::default().fg(app.config.theme.text_color())),
            area,
        );
    }

    fn handle_input(
        &mut self,
        _key: KeyCode,
        _modifiers: KeyModifiers,
        nav_action: Option<crate::navigation::types::NavigationAction>,
    ) -> ModalResult {
        use crate::navigation::types::NavigationAction;
        match nav_action {
            Some(NavigationAction::Cancel) | Some(NavigationAction::Quit) => ModalResult::Closed,
            _ => ModalResult::Continue,
        }
    }

    fn get_title(&self) -> &str {
        "Confirm Deletion"
    }

    fn get_size(&self) -> (u16, u16) {
        (70, 60)
    }
}
