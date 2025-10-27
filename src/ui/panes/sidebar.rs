use crate::app::App;
use ratatui::{layout::Rect, Frame};

pub struct SidebarPane;

impl SidebarPane {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, app: &App, area: Rect) {
        // Placeholder - implement later
    }
}
