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
        frame.size(),
    );

    // Define vertical chunks for status bar, main content, and command bar
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar height
            Constraint::Min(1),    // Main content area (flexible height)
            Constraint::Length(1), // Command bar height
        ])
        .split(frame.size());

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

    // Create status text, including current mode and status message
    let status = Line::from(format!(
        "{} | {}",
        mode,
        app.status_message.as_deref().unwrap_or("")
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
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(10), // Query input height
            Constraint::Length(3),  // Result Tabs height
            Constraint::Min(1),     // Results area
        ])
        .split(area);

    render_query_input(frame, app, chunks[0]);
    render_result_tabs(frame, app, chunks[1]);
    render_results(frame, app, chunks[2]);
}

fn render_query_input(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3), // WHERE clause
            Constraint::Length(3), // ORDER BY clause
            Constraint::Length(3), // Page size and navigation
        ])
        .split(area);

    let query_state = app.current_query_state();

    // WHERE clause
    let mut where_block = Block::default().title("WHERE").borders(Borders::ALL);

    // ORDER BY clause
    let mut order_by_block = Block::default().title("ORDER BY").borders(Borders::ALL);

    // Pagination block
    let mut pagination_block = Block::default().title("Pagination").borders(Borders::ALL);

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

    // Render WHERE clause with cursor if it's the active field
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

    frame.render_widget(
        Paragraph::new(where_content)
            .block(where_block)
            .style(Style::default().fg(app.config.theme.text_color())),
        chunks[0],
    );

    // Render ORDER BY clause with cursor if it's the active field
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

    frame.render_widget(
        Paragraph::new(order_by_content)
            .block(order_by_block)
            .style(Style::default().fg(app.config.theme.text_color())),
        chunks[1],
    );

    // Render pagination info with keybindings hint
    let pagination_info = if let Some(state) = query_state {
        format!(
            "Page: {}/{} | Size: {} | Total: {} | g:First G:Last n:Next p:Prev",
            state.current_page,
            state.total_pages.unwrap_or(1),
            state.page_size,
            state.total_records.unwrap_or(0)
        )
    } else {
        String::from("No active table")
    };

    frame.render_widget(
        Paragraph::new(pagination_info)
            .block(pagination_block)
            .style(Style::default().fg(app.config.theme.text_color())),
        chunks[2],
    );
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
    let mut block = Block::default().title("Results").borders(Borders::ALL);
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

        // Create widths vector with line number column plus data columns
        let mut widths = vec![Constraint::Length(line_num_width + 1)]; // +1 for padding
        let remaining_width = (100 - line_num_width as u16) / header.len() as u16;
        widths.extend(vec![Constraint::Percentage(remaining_width); header.len()]);

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

        let rows: Vec<Row> = result
            .rows
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                let mut row_cells = vec![Cell::from(format!(
                    "{:>width$}",
                    row_idx + 1,
                    width = line_num_width as usize
                ))
                .style(Style::default().fg(app.config.theme.text_color()))];

                // Add the data cells
                row_cells.extend(row.iter().enumerate().map(|(col_idx, cell)| {
                    let is_selected = app.active_pane == Pane::Results
                        && row_idx == app.cursor_position.1
                        && col_idx == app.cursor_position.0;
                    let is_marked = query_state.rows_marked_for_deletion.contains(&row_idx);

                    let style =
                        Style::default()
                            .fg(app.config.theme.text_color())
                            .bg(if is_marked {
                                Color::Rgb(139, 0, 0) // Dark red for marked rows
                            } else if is_selected {
                                app.config.theme.accent_color()
                            } else {
                                app.config.theme.surface0_color()
                            });

                    Cell::from(cell.as_str()).style(style)
                }));

                Row::new(row_cells)
            })
            .collect();

        let table = Table::new(rows)
            .header(header_row)
            .block(block)
            .widths(&widths)
            .style(Style::default().bg(app.config.theme.surface0_color()));

        frame.render_widget(table, area);
    } else {
        frame.render_widget(
            Block::default()
                .title("Results")
                .borders(Borders::ALL)
                .style(
                    Style::default()
                        .fg(app.config.theme.text_color())
                        .bg(app.config.theme.surface0_color()),
                ),
            area,
        );
    }
}

/// Renders the command bar at the bottom of the UI.
fn render_command_bar(frame: &mut Frame, app: &App, area: Rect) {
    let command = if app.input_mode == InputMode::Command {
        format!(":{}", app.command_input) // Show command prefix in command mode
    } else {
        String::new() // Empty in normal/insert mode
    };

    frame.render_widget(
        Paragraph::new(command).style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface0_color()),
        ), // Style with theme colors
        area,
    );
}
