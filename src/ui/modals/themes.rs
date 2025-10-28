use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;
use crate::ui::modal_manager::{Modal, ModalResult};

/// Modal for selecting and switching themes
#[derive(Debug)]
pub struct ThemesModal {
    /// List of available themes
    themes: Vec<String>,
    /// Currently selected theme index
    selected_index: usize,
    /// Current theme name (for highlighting)
    current_theme: String,
}

impl ThemesModal {
    /// Create a new themes modal
    pub fn new(current_theme: String) -> Self {
        let themes = crate::config::Config::list_themes().unwrap_or_default();

        Self {
            themes,
            selected_index: 0,
            current_theme,
        }
    }

    /// Get the currently selected theme
    pub fn selected_theme(&self) -> Option<&String> {
        self.themes.get(self.selected_index)
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.themes.len().saturating_sub(1);
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        if self.selected_index + 1 < self.themes.len() {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    /// Get the selected theme to apply
    pub fn get_theme_to_apply(&self) -> Option<String> {
        self.selected_theme().map(|s| s.clone())
    }
}

impl Modal for ThemesModal {
    fn render(&self, frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
        frame.render_widget(Clear, area);

        // Create modal block
        let block = Block::default()
            .title("Available Themes")
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface1_color()),
            );

        frame.render_widget(block.clone(), area);

        // Get inner area for content
        let inner_area = block.inner(area);

        // Create layout for header and theme list
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(3),    // Theme list
                Constraint::Length(2), // Footer
            ])
            .split(inner_area);

        // Render header
        let header = "Select a theme to switch to:";
        frame.render_widget(
            Paragraph::new(header).style(Style::default().fg(app.config.theme.text_color())),
            chunks[0],
        );

        // Render themes
        if self.themes.is_empty() {
            let no_themes = "No themes available";
            frame.render_widget(
                Paragraph::new(no_themes).style(Style::default().fg(app.config.theme.text_color())),
                chunks[1],
            );
        } else {
            // Create a list of theme items
            let theme_items: Vec<_> = self
                .themes
                .iter()
                .enumerate()
                .map(|(i, theme)| {
                    let is_current = theme == &self.current_theme;
                    let is_selected = i == self.selected_index;

                    let display_text = if is_current {
                        format!("{} (current)", theme)
                    } else {
                        theme.clone()
                    };

                    let style = if is_selected {
                        Style::default()
                            .fg(app.config.theme.base_color())
                            .bg(app.config.theme.accent_color())
                    } else if is_current {
                        Style::default()
                            .fg(app.config.theme.accent_color())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.config.theme.text_color())
                    };

                    ListItem::new(display_text).style(style)
                })
                .collect();

            let list = List::new(theme_items)
                .style(Style::default().bg(app.config.theme.surface1_color()));

            frame.render_widget(list, chunks[1]);
        }

        // Render footer with instructions
        let footer = "Press Enter to apply, Esc to close";
        frame.render_widget(
            Paragraph::new(footer).style(Style::default().fg(app.config.theme.text_color())),
            chunks[2],
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
            Some(NavigationAction::Cancel) | Some(NavigationAction::Quit) => {
                return ModalResult::Closed;
            }
            Some(NavigationAction::MoveUp) => {
                self.move_up();
                return ModalResult::Continue;
            }
            Some(NavigationAction::MoveDown) => {
                self.move_down();
                return ModalResult::Continue;
            }
            Some(NavigationAction::Confirm) => {
                if let Some(theme_name) = self.selected_theme() {
                    return ModalResult::Action(format!("apply_theme:{}", theme_name));
                } else {
                    return ModalResult::Closed;
                }
            }
            _ => {}
        }
        ModalResult::Continue
    }

    fn get_title(&self) -> &str {
        "Themes"
    }

    fn is_blocking(&self) -> bool {
        true
    }

    fn get_size(&self) -> (u16, u16) {
        (50, 60)
    }
}
