use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;
use crate::ui::modal_manager::{Modal, ModalResult};

#[derive(Debug)]
pub struct CommandModal;

impl CommandModal {
    pub fn new() -> Self {
        Self
    }
}

impl Modal for CommandModal {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        frame.render_widget(Clear, area);

        let command_text = format!(">{}", app.command_input);

        let display_text =
            if app.selected_suggestion.is_some() && !app.command_suggestions.is_empty() {
                format!("{} [PREVIEW]", command_text)
            } else {
                command_text
            };

        let command_block = Block::default()
            .title("Command")
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface1_color()),
            );

        frame.render_widget(command_block.clone(), area);

        let inner_area = command_block.inner(area);

        frame.render_widget(
            Paragraph::new(display_text).style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface1_color()),
            ),
            inner_area,
        );

        let cursor_x = inner_area.x + 1 + app.command_input.len() as u16;
        let cursor_y = inner_area.y;
        frame.set_cursor_position(ratatui::layout::Position {
            x: cursor_x,
            y: cursor_y,
        });

        if !app.command_suggestions.is_empty() && !app.command_input.is_empty() {
            self.render_suggestions_dropdown(frame, app, area);
        }
    }

    fn handle_input(
        &mut self,
        _key: KeyCode,
        _modifiers: KeyModifiers,
        nav_action: Option<crate::navigation::types::NavigationAction>,
    ) -> ModalResult {
        if let Some(action) = nav_action {
            use crate::navigation::types::NavigationAction;
            match action {
                NavigationAction::Cancel | NavigationAction::Quit => return ModalResult::Closed,
                _ => {}
            }
        }
        ModalResult::Continue
    }

    fn get_title(&self) -> &str {
        "Command"
    }

    fn get_size(&self) -> (u16, u16) {
        (60, 5)
    }
}

impl CommandModal {
    fn render_suggestions_dropdown(&self, frame: &mut Frame, app: &App, command_area: Rect) {
        let dropdown_height = std::cmp::min(app.command_suggestions.len() as u16, 6);
        let dropdown_y = command_area.y + command_area.height;
        let command_text_offset = 2;
        let dropdown_x = command_area.x + command_text_offset;
        let dropdown_width = command_area.width.saturating_sub(command_text_offset);

        let dropdown_y = std::cmp::min(
            dropdown_y,
            frame.area().height.saturating_sub(dropdown_height),
        );

        let dropdown_area = Rect {
            x: dropdown_x,
            y: dropdown_y,
            width: dropdown_width,
            height: dropdown_height,
        };

        frame.render_widget(Clear, dropdown_area);

        const VISIBLE_ITEMS: usize = 6;
        let total_items = app.command_suggestions.len();
        let scroll_offset = app.suggestions_scroll_offset;

        let visible_suggestions: Vec<ListItem> = app
            .command_suggestions
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(VISIBLE_ITEMS)
            .map(|(idx, suggestion)| {
                let style = if Some(idx) == app.selected_suggestion {
                    Style::default()
                        .fg(app.config.theme.base_color())
                        .bg(app.config.theme.accent_color())
                } else {
                    Style::default()
                        .fg(app.config.theme.text_color())
                        .bg(app.config.theme.surface1_color())
                };
                ListItem::new(suggestion.as_str()).style(style)
            })
            .collect();

        let suggestions_list = List::new(visible_suggestions)
            .style(Style::default().bg(app.config.theme.surface1_color()));

        frame.render_widget(suggestions_list, dropdown_area);

        if total_items > VISIBLE_ITEMS {
            self.render_scrollbar(frame, app, dropdown_area, total_items, VISIBLE_ITEMS);
        }
    }

    fn render_scrollbar(
        &self,
        frame: &mut Frame,
        app: &App,
        dropdown_area: Rect,
        total_items: usize,
        visible_items: usize,
    ) {
        if total_items <= visible_items {
            return;
        }

        let scrollbar_width = 1;
        let scrollbar_x = dropdown_area.x + dropdown_area.width - scrollbar_width;
        let scrollbar_area = Rect {
            x: scrollbar_x,
            y: dropdown_area.y,
            width: scrollbar_width,
            height: dropdown_area.height,
        };

        let thumb_height =
            ((visible_items as f32 / total_items as f32) * dropdown_area.height as f32) as u16;
        let thumb_height = thumb_height.max(1);

        let scroll_progress =
            app.suggestions_scroll_offset as f32 / (total_items - visible_items) as f32;
        let thumb_y = dropdown_area.y
            + (scroll_progress * (dropdown_area.height - thumb_height) as f32) as u16;

        let track_area = Rect {
            x: scrollbar_area.x,
            y: scrollbar_area.y,
            width: scrollbar_area.width,
            height: scrollbar_area.height,
        };

        frame.render_widget(
            Block::default().style(Style::default().bg(app.config.theme.surface2_color())),
            track_area,
        );

        let thumb_area = Rect {
            x: scrollbar_area.x,
            y: thumb_y,
            width: scrollbar_area.width,
            height: thumb_height,
        };

        frame.render_widget(
            Block::default().style(Style::default().bg(app.config.theme.accent_color())),
            thumb_area,
        );
    }
}
