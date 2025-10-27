use ratatui::{Frame, layout::Rect};
use crate::app::App;

pub struct ResultsPane;

impl ResultsPane {
    pub fn new() -> Self {
        Self
    }
    
    pub fn render(&self, frame: &mut Frame, app: &App, area: Rect) {
        // Placeholder - implement later
    }
}

