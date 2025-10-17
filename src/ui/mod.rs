use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Tabs},
    Frame,
};

use ratatui::layout::Direction as LayoutDirection;

use crate::app::{App, InputMode};
use crate::database::ConnectionStatus;
use crate::logging;

mod modal;
pub mod types;

pub use modal::{render_connection_modal, render_deletion_modal};
pub use types::Pane;

/// Renders the entire UI of the application.
pub fn render(frame: &mut Frame, app: &App) {
    // Set background color for the entire frame
    frame.render_widget(
        Block::default().style(Style::default().bg(app.config.theme.base_color())),
        frame.area(),
    );

    // Define vertical chunks for status bar, main content, and command bar
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar height
            Constraint::Min(1),    // Main content area (flexible height)
            Constraint::Length(1), // Command bar height
        ])
        .split(frame.area());

    render_status_bar(frame, app, chunks[0]);
    render_main_content(frame, app, chunks[1]);
    render_command_bar(frame, app, chunks[2]);

    // Render appropriate modal if active
    if app.show_connection_modal {
        render_connection_modal(frame, app);
    } else if app.show_deletion_modal {
        render_deletion_modal(frame, app);
    }
}

/// Renders the status bar at the top of the UI.
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode = match app.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Insert => "INSERT",
        InputMode::Command => "COMMAND",
    };

    // Create status text, including current mode, theme, and status message
    let theme_info = if app.input_mode == InputMode::Command && app.selected_suggestion.is_some() {
        if let Some(suggestion) = app.get_selected_suggestion() {
            if suggestion.starts_with("theme ") {
                let theme_name = suggestion.strip_prefix("theme ").unwrap_or("");
                format!(" | Theme: {} (preview)", theme_name)
            } else {
                format!(" | Theme: {}", app.get_current_theme_name())
            }
        } else {
            format!(" | Theme: {}", app.get_current_theme_name())
        }
    } else {
        format!(" | Theme: {}", app.get_current_theme_name())
    };

    let status = Line::from(format!(
        "{} | {}{}",
        mode,
        app.status_message.as_deref().unwrap_or(""),
        theme_info
    ));

    frame.render_widget(
        Paragraph::new(status).style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface0_color()),
        ), // Style with theme colors
        area,
    );
}

/// Renders the main content area, split into sidebar and main panel.
fn render_main_content(frame: &mut Frame, app: &App, area: Rect) {
    // Split main area horizontally into sidebar (connections) and main panel (query, results)
    let horizontal_chunks = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Sidebar takes 20% width
            Constraint::Percentage(80), // Main panel takes 80% width
        ])
        .split(area);

    render_sidebar(frame, app, horizontal_chunks[0]); // Render the sidebar (connections tree)
    render_main_panel(frame, app, horizontal_chunks[1]); // Render the main panel (query input, results)
}

/// Renders the sidebar, displaying the connections tree.
pub fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    logging::debug(&format!(
        "Rendering sidebar with {} connections",
        app.connection_tree.len()
    ))
    .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

    // Split sidebar vertically into header and connection tree area
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3), // Header area height
            Constraint::Min(1),    // Connection tree area (flexible height)
        ])
        .split(area);

    // Render header paragraph
    let header = Line::from("Connections (press 'a' to add)");
    let mut block = Block::default().borders(Borders::ALL).style(
        Style::default()
            .fg(app.config.theme.text_color())
            .bg(app.config.theme.surface0_color()),
    );

    // Highlight the block if it's the active pane
    if app.active_pane == Pane::Connections {
        block = block.border_style(Style::default().fg(app.config.theme.accent_color()));
    }

    frame.render_widget(Paragraph::new(header).block(block), chunks[0]);

    // Prepare tree items for the connection tree list
    let mut tree_items = Vec::new();
    let mut visible_index = 0; // Track visible index for highlighting selected item

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

        // Connection item line
        let expanded_symbol = if connection.is_expanded { "▼" } else { "▶" }; // Symbol based on expansion state
        let status_symbol = match connection.status {
            // Status symbol based on connection status
            ConnectionStatus::Connected => "●",
            ConnectionStatus::Connecting => "◌",
            ConnectionStatus::Failed => "✗",
            ConnectionStatus::NotConnected => "○",
        };

        // Style for connection item, highlight if selected
        let conn_style = if app.highlight_selected_item(visible_index) {
            Style::default()
                .fg(app.config.theme.accent_color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.config.theme.text_color())
        };

        tree_items.push(ListItem::new(Line::from(vec![
            Span::raw(format!("{} ", expanded_symbol)), // Expansion symbol
            Span::styled(
                status_symbol,
                match connection.status {
                    // Status symbol with color
                    ConnectionStatus::Connected => Style::default().fg(Color::Green),
                    ConnectionStatus::Connecting => Style::default().fg(Color::Yellow),
                    ConnectionStatus::Failed => Style::default().fg(Color::Red),
                    ConnectionStatus::NotConnected => Style::default().fg(Color::Gray),
                },
            ),
            Span::raw(" "),
            Span::styled(&connection.connection_config.name, conn_style), // Connection name
        ])));
        visible_index += 1; // Increment visible index after connection

        // Render databases if connection is expanded
        if connection.is_expanded {
            logging::debug(&format!(
                "Connection {} is expanded, showing {} databases",
                connection.connection_config.name,
                connection.databases.len()
            ))
            .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

            for (db_idx, database) in connection.databases.iter().enumerate() {
                let db_expanded = if database.is_expanded { "▼" } else { "▶" };

                let db_style = if app.highlight_selected_item(visible_index) {
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
                visible_index += 1; // Increment visible index after database

                // Render schemas if database is expanded
                if database.is_expanded {
                    for (schema_idx, schema) in database.schemas.iter().enumerate() {
                        let schema_expanded = if schema.is_expanded { "▼" } else { "▶" }; // Expansion symbol for schema
                        let schema_style = if app.highlight_selected_item(visible_index) {
                            // Style for schema item, highlight if selected
                            Style::default()
                                .fg(app.config.theme.accent_color())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.config.theme.text_color())
                        };
                        tree_items.push(ListItem::new(Line::from(vec![
                            Span::raw("    "),                        // Indentation
                            Span::raw(schema_expanded),               // Schema expansion symbol
                            Span::raw(" 📁 "),                        // Schema icon
                            Span::styled(&schema.name, schema_style), // Schema name
                        ])));
                        visible_index += 1; // Increment visible index after schema

                        // Render tables if schema is expanded
                        if schema.is_expanded {
                            for table in &schema.tables {
                                let table_style = if app.highlight_selected_item(visible_index) {
                                    // Style for table item, highlight if selected
                                    Style::default()
                                        .fg(app.config.theme.accent_color())
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(app.config.theme.text_color())
                                };
                                tree_items.push(ListItem::new(Line::from(vec![
                                    Span::raw("      "),              // Indentation
                                    Span::raw("📋 "),                 // Table icon
                                    Span::styled(table, table_style), // Table name
                                ])));
                                visible_index += 1; // Increment visible index after table
                            }
                        }
                    }
                }
            }
        }
    }

    // Render the connection tree list
    frame.render_widget(
        List::new(tree_items)
            .block(Block::default().borders(Borders::ALL)) // Bordered block
            .style(Style::default().bg(app.config.theme.surface0_color())), // Style with theme colors
        chunks[1],
    );
}

fn render_main_panel(frame: &mut Frame, app: &App, area: Rect) {
    // Set background color for the main panel area
    frame.render_widget(
        Block::default().style(Style::default().bg(app.config.theme.base_color())),
        area,
    );

    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(1)])
        .split(area);

    render_query_input(frame, app, chunks[0]);
    render_results_panel(frame, app, chunks[1]);
}

fn render_query_input(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3), // WHERE clause
            Constraint::Length(3), // ORDER BY clause
        ])
        .split(area);

    let query_state = app.current_query_state();

    // WHERE clause
    let mut where_block = Block::default()
        .title("WHERE")
        .borders(Borders::ALL)
        .title_style(
            Style::default()
                .fg(app.config.theme.header_fg_color())
                .bg(app.config.theme.header_bg_color()),
        )
        .style(Style::default().bg(app.config.theme.surface0_color()));

    // ORDER BY clause
    let mut order_by_block = Block::default()
        .title("ORDER BY")
        .borders(Borders::ALL)
        .title_style(
            Style::default()
                .fg(app.config.theme.header_fg_color())
                .bg(app.config.theme.header_bg_color()),
        )
        .style(Style::default().bg(app.config.theme.surface0_color()));

    // If query input is active pane, highlight the current field
    if app.active_pane == Pane::QueryInput {
        match app.cursor_position.0 {
            0 => {
                where_block =
                    where_block.border_style(Style::default().fg(app.config.theme.accent_color()))
            }
            1 => {
                order_by_block = order_by_block
                    .border_style(Style::default().fg(app.config.theme.accent_color()))
            }
            _ => {}
        }
    }

    // Render WHERE clause with cursor when inserting in WHERE
    let where_content = if let Some(state) = query_state {
        if app.active_pane == Pane::QueryInput
            && app.cursor_position.0 == 0
            && app.input_mode == InputMode::Insert
        {
            let mut content = state.where_clause.clone();
            content.insert(app.cursor_position.1, '|'); // Add cursor
            content
        } else {
            state.where_clause.clone()
        }
    } else {
        String::new()
    };

    let where_style = if app.active_pane == Pane::QueryInput
        && app.cursor_position.0 == 0
        && app.input_mode == InputMode::Insert
    {
        Style::default()
            .fg(app.config.theme.text_color())
            .bg(app.config.theme.cursor_color())
    } else {
        Style::default().fg(app.config.theme.text_color())
    };

    frame.render_widget(
        Paragraph::new(where_content)
            .block(where_block)
            .style(where_style),
        chunks[0],
    );

    // Render ORDER BY clause with cursor when inserting in ORDER BY
    let order_by_content = if let Some(state) = query_state {
        if app.active_pane == Pane::QueryInput
            && app.cursor_position.0 == 1
            && app.input_mode == InputMode::Insert
        {
            let mut content = state.order_by_clause.clone();
            content.insert(app.cursor_position.1, '|'); // Add cursor
            content
        } else {
            state.order_by_clause.clone()
        }
    } else {
        String::new()
    };

    let order_by_style = if app.active_pane == Pane::QueryInput
        && app.cursor_position.0 == 1
        && app.input_mode == InputMode::Insert
    {
        Style::default()
            .fg(app.config.theme.text_color())
            .bg(app.config.theme.cursor_color())
    } else {
        Style::default().fg(app.config.theme.text_color())
    };

    frame.render_widget(
        Paragraph::new(order_by_content)
            .block(order_by_block)
            .style(order_by_style),
        chunks[1],
    );

    // Show terminal cursor as a block in NORMAL mode at the query cursor position
    if app.active_pane == Pane::QueryInput && app.input_mode == InputMode::Normal {
        // Compute inner areas to align cursor properly inside borders
        let where_inner = Block::default().borders(Borders::ALL).inner(chunks[0]);
        let order_by_inner = Block::default().borders(Borders::ALL).inner(chunks[1]);

        match app.cursor_position.0 {
            0 => {
                let x = where_inner.x
                    + app.cursor_position.1.min(app.get_current_field_length()) as u16;
                let y = where_inner.y;
                frame.set_cursor_position(ratatui::layout::Position { x, y });
            }
            1 => {
                let x = order_by_inner.x
                    + app.cursor_position.1.min(app.get_current_field_length()) as u16;
                let y = order_by_inner.y;
                frame.set_cursor_position(ratatui::layout::Position { x, y });
            }
            _ => {}
        }
    }
}

fn render_results_panel(frame: &mut Frame, app: &App, area: Rect) {
    let has_tabs = !app.result_tabs.is_empty();
    let constraints = if has_tabs {
        vec![
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ]
    } else {
        vec![Constraint::Min(1), Constraint::Length(3)]
    };

    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints(constraints)
        .split(area);

    let mut index = 0;
    if has_tabs {
        render_result_tabs(frame, app, chunks[index]);
        index += 1;
    }

    render_results(frame, app, chunks[index]);
    index += 1;
    render_pagination(frame, app, chunks[index]);
}

/// Renders the result tabs if there are any result sets.
fn render_result_tabs(frame: &mut Frame, app: &App, area: Rect) {
    if app.result_tabs.is_empty() {
        return;
    }

    let tab_titles: Vec<Line> = app
        .result_tabs
        .iter()
        .map(|(name, _, _)| Line::from(name.clone()))
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(app.selected_result_tab_index.unwrap_or(0))
        .block(Block::default().borders(Borders::BOTTOM))
        .style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface0_color()),
        )
        .highlight_style(
            Style::default()
                .fg(app.config.theme.accent_color())
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" | "));

    frame.render_widget(tabs, area);
}

/// Renders the results table.
fn render_results(frame: &mut Frame, app: &App, area: Rect) {
    let mut block = Block::default()
        .title("Results")
        .borders(Borders::ALL)
        .title_style(
            Style::default()
                .fg(app.config.theme.header_fg_color())
                .bg(app.config.theme.header_bg_color()),
        );
    if app.active_pane == Pane::Results {
        block = block.border_style(Style::default().fg(app.config.theme.accent_color()));
    }
    let current_result = app.selected_result_tab_index.and_then(|tab_index| {
        app.result_tabs
            .get(tab_index)
            .map(|(_, result, state)| (result, state))
    });
    if let Some((result, query_state)) = current_result {
        let header = result
            .columns
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>();
        if header.is_empty() {
            frame.render_widget(
                Paragraph::new("No results to display.")
                    .style(Style::default().fg(app.config.theme.text_color())),
                area,
            );
            return;
        }

        // Calculate the width needed for the line number column
        let max_lines = result.rows.len();
        let line_num_width = max_lines.to_string().len().max(3) as u16; // Minimum width of 3

        // Compute exact column widths based on available inner width
        let spacing: u16 = 1;
        let table_inner = block.inner(area);
        let first_col_w = line_num_width + 1; // +1 for padding
        let mut widths: Vec<Constraint> = Vec::with_capacity(1 + header.len());
        widths.push(Constraint::Length(first_col_w));
        let data_cols = header.len() as u16;
        if data_cols > 0 {
            let total_spacing = spacing.saturating_mul(data_cols.saturating_sub(1));
            let remaining_w = table_inner
                .width
                .saturating_sub(first_col_w)
                .saturating_sub(total_spacing);
            let base = if data_cols > 0 {
                remaining_w / data_cols
            } else {
                0
            };
            let rem = if data_cols > 0 {
                remaining_w % data_cols
            } else {
                0
            };
            for i in 0..data_cols {
                let w = base + if i < rem { 1 } else { 0 };
                widths.push(Constraint::Length(w));
            }
        }

        // Create header cells including line number column
        let mut header_cells = vec![Cell::from("#").style(
            Style::default()
                .fg(app.config.theme.accent_color())
                .add_modifier(Modifier::BOLD),
        )];

        // Add the regular headers
        header_cells.extend(header.iter().map(|&h| {
            Cell::from(h).style(
                Style::default()
                    .fg(app.config.theme.accent_color())
                    .add_modifier(Modifier::BOLD),
            )
        }));

        let header_row = Row::new(header_cells);

        // Determine how many rows can be displayed and the starting index to keep cursor visible
        let header_rows: u16 = 1;
        let visible_capacity = table_inner.height.saturating_sub(header_rows).max(0) as usize;

        let total_rows = result.rows.len();
        let max_start = total_rows.saturating_sub(visible_capacity);
        let cursor_row = app.cursor_position.1.min(total_rows.saturating_sub(1));
        let start_row = if visible_capacity == 0 {
            0
        } else {
            cursor_row
                .saturating_sub(visible_capacity / 2)
                .min(max_start)
        };

        let rows: Vec<Row> = result
            .rows
            .iter()
            .enumerate()
            .skip(start_row)
            .take(visible_capacity)
            .map(|(row_idx, row)| {
                let is_marked = query_state.rows_marked_for_deletion.contains(&row_idx);
                let is_selected =
                    app.active_pane == Pane::Results && row_idx == app.cursor_position.1;

                let base_bg = if is_marked {
                    Color::Rgb(139, 0, 0) // Dark red for marked rows
                } else if is_selected {
                    app.config.theme.accent_color()
                } else if (row_idx + start_row) % 2 == 0 {
                    app.config.theme.row_even_bg_color()
                } else {
                    app.config.theme.row_odd_bg_color()
                };

                let mut row_cells = vec![Cell::from(format!(
                    "{:>width$}",
                    row_idx + 1,
                    width = line_num_width as usize
                ))
                .style(
                    Style::default()
                        .fg(app.config.theme.text_color())
                        .bg(base_bg),
                )];

                // Add the data cells
                row_cells.extend(row.iter().enumerate().map(|(col_idx, cell)| {
                    let is_selected = app.active_pane == Pane::Results
                        && row_idx == app.cursor_position.1
                        && col_idx == app.cursor_position.0;
                    let is_marked = query_state.rows_marked_for_deletion.contains(&row_idx);

                    let base_bg = if is_marked {
                        Color::Rgb(139, 0, 0) // Dark red for marked rows
                    } else if is_selected {
                        app.config.theme.accent_color()
                    } else if (row_idx + start_row) % 2 == 0 {
                        app.config.theme.row_even_bg_color()
                    } else {
                        app.config.theme.row_odd_bg_color()
                    };

                    let style = Style::default()
                        .fg(app.config.theme.text_color())
                        .bg(base_bg);

                    Cell::from(cell.as_str()).style(style)
                }));

                Row::new(row_cells)
            })
            .collect();

        let table = Table::new(rows, widths)
            .header(header_row)
            .block(block)
            .column_spacing(spacing)
            .style(Style::default().bg(app.config.theme.surface0_color()));

        frame.render_widget(table, area);
    } else {
        frame.render_widget(
            Block::default()
                .title("Results")
                .borders(Borders::ALL)
                .title_style(
                    Style::default()
                        .fg(app.config.theme.header_fg_color())
                        .bg(app.config.theme.header_bg_color()),
                )
                .style(
                    Style::default()
                        .fg(app.config.theme.text_color())
                        .bg(app.config.theme.surface0_color()),
                ),
            area,
        );
    }
}

fn render_pagination(frame: &mut Frame, app: &App, area: Rect) {
    let mut block = Block::default()
        .title("Pagination")
        .borders(Borders::ALL)
        .title_style(
            Style::default()
                .fg(app.config.theme.header_fg_color())
                .bg(app.config.theme.header_bg_color()),
        )
        .style(Style::default().bg(app.config.theme.surface0_color()));
    if app.active_pane == Pane::Results {
        block = block.border_style(Style::default().fg(app.config.theme.accent_color()));
    }

    let pagination_info = if let Some(state) = app.current_query_state() {
        format!(
            "Page: {}/{} | Size: {} | Total: {} | {}:First {}:Last {}:Prev {}:Next ",
            state.current_page,
            state.total_pages.unwrap_or(1),
            state.page_size,
            state.total_records.unwrap_or(0),
            app.config.keymap.first_page_key,
            app.config.keymap.last_page_key,
            app.config.keymap.prev_page_key,
            app.config.keymap.next_page_key,
        )
    } else {
        String::from("No active table")
    };

    frame.render_widget(
        Paragraph::new(pagination_info)
            .block(block)
            .style(Style::default().fg(app.config.theme.text_color())),
        area,
    );
}

/// Renders the command bar at the bottom of the UI.
fn render_command_bar(frame: &mut Frame, app: &App, area: Rect) {
    if app.input_mode == InputMode::Command {
        // Split area into command input and suggestions
        let chunks = Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(1), // Command input
                Constraint::Length(5), // Suggestions (show more)
            ])
            .split(area);

        let command = format!(":{}", app.command_input);

        // Render command input
        frame.render_widget(
            Paragraph::new(command).style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface0_color()),
            ),
            chunks[0],
        );

        // Render suggestions
        if !app.command_suggestions.is_empty() {
            let suggestion_items: Vec<ListItem> = app
                .command_suggestions
                .iter()
                .enumerate()
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

            let suggestions_list = List::new(suggestion_items)
                .style(Style::default().bg(app.config.theme.surface1_color()));

            frame.render_widget(suggestions_list, chunks[1]);
        }

        // Place the terminal cursor at the end of the command input
        let cursor_x = area.x + 1 + app.command_input.len() as u16; // account for ':'
        let cursor_y = area.y;
        frame.set_cursor_position(ratatui::layout::Position {
            x: cursor_x,
            y: cursor_y,
        });
    } else {
        // Empty command bar in normal mode
        frame.render_widget(
            Paragraph::new("").style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface0_color()),
            ),
            area,
        );
    }
}
