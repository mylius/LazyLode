use crate::app::App;
use crate::navigation::types::NavigationAction;
use crate::ui::components::{FieldNavigator, TextInput};
use crate::ui::types::Pane;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph as Widget},
    Frame,
};

pub struct QueryInputPane {
    where_clause: TextInput,
    order_by_clause: TextInput,
    field_navigator: FieldNavigator,
}

impl QueryInputPane {
    pub fn new() -> Self {
        Self {
            where_clause: TextInput::new(),
            order_by_clause: TextInput::new(),
            field_navigator: FieldNavigator::new(2),
        }
    }

    pub fn current_vim_mode(&self) -> crate::navigation::types::VimMode {
        let current = self.field_navigator.current_field();
        match current {
            0 => self.where_clause.mode(),
            1 => self.order_by_clause.mode(),
            _ => crate::navigation::types::VimMode::Normal,
        }
    }

    pub fn render(&self, frame: &mut Frame, app: &App, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)])
            .split(area);

        let is_active = app.active_pane == Pane::QueryInput;
        let current_field = self.field_navigator.current_field();

        // Render WHERE clause
        self.render_field(
            frame,
            chunks[0],
            "WHERE",
            &self.where_clause,
            0,
            current_field,
            is_active,
            app,
        );

        // Render ORDER BY clause
        self.render_field(
            frame,
            chunks[1],
            "ORDER BY",
            &self.order_by_clause,
            1,
            current_field,
            is_active,
            app,
        );
    }

    fn render_field(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        text_input: &TextInput,
        field_index: usize,
        current_field: usize,
        is_active: bool,
        app: &App,
    ) {
        let mut block = Block::default()
            .title(format!(
                "{}{}",
                title,
                if is_active && field_index == current_field {
                    " [ACTIVE]"
                } else {
                    ""
                }
            ))
            .borders(Borders::ALL)
            .title_style(
                Style::default()
                    .fg(app.config.theme.header_fg_color())
                    .bg(app.config.theme.header_bg_color()),
            )
            .style(Style::default().bg(app.config.theme.surface0_color()));

        if is_active && field_index == current_field {
            block = block.border_style(Style::default().fg(app.config.theme.accent_color()));
        }

        let content = if is_active
            && field_index == current_field
            && text_input.mode() == crate::navigation::types::VimMode::Insert
        {
            text_input.display_text_with_cursor()
        } else {
            text_input.content().to_string()
        };

        frame.render_widget(
            Widget::new(content)
                .block(block)
                .style(Style::default().fg(app.config.theme.text_color())),
            area,
        );

        // In normal mode, show terminal cursor
        if is_active
            && field_index == current_field
            && text_input.mode() == crate::navigation::types::VimMode::Normal
        {
            let inner = Block::default().borders(Borders::ALL).inner(area);
            let cursor_pos = text_input.cursor_position().min(text_input.content().len());
            let cursor_x = inner.x + cursor_pos as u16;
            let cursor_y = inner.y;
            frame.set_cursor_position(ratatui::layout::Position {
                x: cursor_x,
                y: cursor_y,
            });
        }
    }

    pub fn handle_input(
        &mut self,
        key: KeyCode,
        _modifiers: KeyModifiers,
        nav_action: Option<NavigationAction>,
    ) -> bool {
        let current = self.field_navigator.current_field();
        let current_input = if current == 0 {
            &mut self.where_clause
        } else {
            &mut self.order_by_clause
        };

        // Handle Confirm action (Enter key) to trigger query execution
        if let Some(NavigationAction::Confirm) = nav_action {
            // In insert mode, Enter should execute the query
            return false; // Let it fall through to handler
        }

        // Handle Enter key in insert mode - should trigger query execution
        if key == KeyCode::Enter
            && current_input.mode() == crate::navigation::types::VimMode::Insert
        {
            return false; // Return false to let the handler execute the query
        }

        // Try handling all keys through VimEditor first
        if current_input.handle_key(key, _modifiers) {
            return true;
        }

        // Handle field navigation
        if let Some(action) = nav_action {
            if current_input.mode() == crate::navigation::types::VimMode::Normal {
                if self.field_navigator.handle_action(action) {
                    return true;
                }
            }
        }

        false
    }

    pub fn get_where_content(&self) -> String {
        self.where_clause.content().to_string()
    }

    pub fn get_order_by_content(&self) -> String {
        self.order_by_clause.content().to_string()
    }

    pub fn exit_insert_mode(&mut self) {
        self.where_clause
            .set_mode(crate::navigation::types::VimMode::Normal);
        self.order_by_clause
            .set_mode(crate::navigation::types::VimMode::Normal);
    }
}
