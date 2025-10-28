use crate::app::App;
use crate::database::ConnectionStatus;
use crate::logging;
use crate::ui::types::Pane;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub struct SidebarPane;

impl SidebarPane {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, app: &App, area: Rect) {
        logging::debug(&format!(
            "Rendering sidebar with {} connections",
            app.connection_tree.len()
        ))
        .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        let nav_info = if app.active_pane == Pane::Connections {
            format!(" [{}]", app.navigation_manager.get_navigation_info())
        } else {
            String::new()
        };

        let header = Line::from(format!("Connections (press 'a' to add){}", nav_info));
        let header_block = Block::default().borders(Borders::ALL).style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface0_color()),
        );

        frame.render_widget(Paragraph::new(header).block(header_block), chunks[0]);

        let mut tree_items = Vec::new();
        let mut current_visual_index = 0;

        for (conn_idx, connection) in app.connection_tree.iter().enumerate() {
            logging::debug(&format!(
                "Rendering connection {}: {} (expanded: {}, status: {:?}, databases: {})",
                conn_idx,
                connection.connection_config.name,
                connection.is_expanded,
                connection.status,
                connection.databases.len()
            ))
            .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

            let expanded_symbol = if connection.is_expanded { "▼" } else { "▶" };
            let status_symbol = match connection.status {
                ConnectionStatus::Connected => "●",
                ConnectionStatus::Connecting => "◌",
                ConnectionStatus::Failed => "✗",
                ConnectionStatus::NotConnected => "○",
            };

            let conn_style = if app.highlight_selected_item(current_visual_index) {
                Style::default()
                    .fg(app.config.theme.accent_color())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.config.theme.text_color())
            };

            tree_items.push(ListItem::new(Line::from(vec![
                Span::raw(format!("{} ", expanded_symbol)),
                Span::styled(
                    status_symbol,
                    match connection.status {
                        ConnectionStatus::Connected => Style::default().fg(Color::Green),
                        ConnectionStatus::Connecting => Style::default().fg(Color::Yellow),
                        ConnectionStatus::Failed => Style::default().fg(Color::Red),
                        ConnectionStatus::NotConnected => Style::default().fg(Color::Gray),
                    },
                ),
                Span::raw(" "),
                Span::styled(&connection.connection_config.name, conn_style),
            ])));

            current_visual_index += 1;

            if connection.is_expanded {
                logging::debug(&format!(
                    "Connection {} is expanded, showing {} databases",
                    connection.connection_config.name,
                    connection.databases.len()
                ))
                .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

                for (_db_idx, database) in connection.databases.iter().enumerate() {
                    let db_expanded = if database.is_expanded { "▼" } else { "▶" };

                    let db_style = if app.highlight_selected_item(current_visual_index) {
                        Style::default()
                            .fg(app.config.theme.accent_color())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.config.theme.text_color())
                    };

                    tree_items.push(ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        Span::raw(db_expanded),
                        Span::raw(" 🗄 "),
                        Span::styled(&database.name, db_style),
                    ])));

                    current_visual_index += 1;

                    if database.is_expanded {
                        for (_schema_idx, schema) in database.schemas.iter().enumerate() {
                            let schema_expanded = if schema.is_expanded { "▼" } else { "▶" };

                            let schema_style = if app.highlight_selected_item(current_visual_index)
                            {
                                Style::default()
                                    .fg(app.config.theme.accent_color())
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(app.config.theme.text_color())
                            };

                            tree_items.push(ListItem::new(Line::from(vec![
                                Span::raw("    "),
                                Span::raw(schema_expanded),
                                Span::raw(" 📁 "),
                                Span::styled(&schema.name, schema_style),
                            ])));

                            current_visual_index += 1;

                            if schema.is_expanded {
                                for table in &schema.tables {
                                    let table_style =
                                        if app.highlight_selected_item(current_visual_index) {
                                            Style::default()
                                                .fg(app.config.theme.accent_color())
                                                .add_modifier(Modifier::BOLD)
                                        } else {
                                            Style::default().fg(app.config.theme.text_color())
                                        };

                                    tree_items.push(ListItem::new(Line::from(vec![
                                        Span::raw("      "),
                                        Span::raw("📋 "),
                                        Span::styled(table, table_style),
                                    ])));

                                    current_visual_index += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut tree_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(app.config.theme.surface0_color()));

        if app.active_pane == Pane::Connections {
            tree_block =
                tree_block.border_style(Style::default().fg(app.config.theme.accent_color()));
        }

        frame.render_widget(
            List::new(tree_items)
                .block(tree_block)
                .style(Style::default().bg(app.config.theme.surface0_color())),
            chunks[1],
        );
    }
}
