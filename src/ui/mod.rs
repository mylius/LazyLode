use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs},
    Frame,
};

use ratatui::layout::Direction as LayoutDirection;

use crate::app::{App, InputMode};
use crate::database::ConnectionStatus;
use crate::logging;

pub mod layout;
mod modal;
pub mod types;

#[cfg(test)]
mod tab_shortening_tests;

pub use modal::{render_connection_modal, render_deletion_modal, render_themes_modal};
pub use types::Pane;

/// Renders the entire UI of the application.
pub fn render(frame: &mut Frame, app: &App) {
    // Set background color for the entire frame
    frame.render_widget(
        Block::default().style(Style::default().bg(app.config.theme.base_color())),
        frame.area(),
    );

    // Define vertical chunks for status bar and main content (no command bar)
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar height
            Constraint::Min(1),    // Main content area (flexible height)
        ])
        .split(frame.area());

    render_status_bar(frame, app, chunks[0]);
    render_main_content(frame, app, chunks[1]);

    // Render appropriate modal if active
    if app.show_connection_modal {
        render_connection_modal(frame, app);
    } else if app.show_deletion_modal {
        render_deletion_modal(frame, app);
    } else if app.show_themes_modal {
        render_themes_modal(frame, app);
    }

    // Render floating command window if in command mode
    if app.input_mode == InputMode::Command {
        render_floating_command_window(frame, app);
    }
}

/// Renders the status bar at the top of the UI.
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    // Use new navigation system mode indicator
    let mode = app.navigation_manager.get_mode_indicator();

    // Determine what to show in the navigation info section
    let nav_info = if app.input_mode == InputMode::Command {
        "Command".to_string()
    } else if app.show_connection_modal {
        "Add Connection".to_string()
    } else if app.show_deletion_modal {
        "Delete Confirmation".to_string()
    } else if app.show_themes_modal {
        "Themes".to_string()
    } else {
        app.navigation_manager.get_navigation_info()
    };

    // Create status text, including current mode, navigation info, and status message
    let status = Line::from(format!(
        "{} | {} | {}",
        mode,
        nav_info,
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

    // Show navigation info if this is the active pane
    let nav_info = if app.active_pane == Pane::Connections {
        format!(" [{}]", app.navigation_manager.get_navigation_info())
    } else {
        String::new()
    };

    // Render header paragraph
    let header = Line::from(format!("Connections (press 'a' to add){}", nav_info));
    let header_block = Block::default().borders(Borders::ALL).style(
        Style::default()
            .fg(app.config.theme.text_color())
            .bg(app.config.theme.surface0_color()),
    );

    frame.render_widget(Paragraph::new(header).block(header_block), chunks[0]);

    // Prepare tree items for the connection tree list
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

        // Connection item line
        let expanded_symbol = if connection.is_expanded { "▼" } else { "▶" }; // Symbol based on expansion state
        let status_symbol = match connection.status {
            // Status symbol based on connection status
            ConnectionStatus::Connected => "●",
            ConnectionStatus::Connecting => "◌",
            ConnectionStatus::Failed => "✗",
            ConnectionStatus::NotConnected => "○",
        };

        // Style for connection item, highlight if selected using visual index
        let conn_style = if app.highlight_selected_item(current_visual_index) {
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

        // Increment visual index after rendering connection
        current_visual_index += 1;

        // Render databases if connection is expanded
        if connection.is_expanded {
            logging::debug(&format!(
                "Connection {} is expanded, showing {} databases",
                connection.connection_config.name,
                connection.databases.len()
            ))
            .unwrap_or_else(|e| eprintln!("Logging error: {}", e));

            for (_db_idx, database) in connection.databases.iter().enumerate() {
                let db_expanded = if database.is_expanded { "▼" } else { "▶" };

                // Style for database item, highlight if selected using visual index
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

                // Increment visual index after rendering database
                current_visual_index += 1;

                // Render schemas if database is expanded
                if database.is_expanded {
                    for (_schema_idx, schema) in database.schemas.iter().enumerate() {
                        let schema_expanded = if schema.is_expanded { "▼" } else { "▶" }; // Expansion symbol for schema

                        // Style for schema item, highlight if selected using visual index
                        let schema_style = if app.highlight_selected_item(current_visual_index) {
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

                        // Increment visual index after rendering schema
                        current_visual_index += 1;

                        // Render tables if schema is expanded
                        if schema.is_expanded {
                            for table in &schema.tables {
                                // Style for table item, highlight if selected using visual index
                                let table_style =
                                    if app.highlight_selected_item(current_visual_index) {
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

                                // Increment visual index after rendering table
                                current_visual_index += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Render the connection tree list
    let mut tree_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(app.config.theme.surface0_color()));

    if app.active_pane == Pane::Connections {
        tree_block = tree_block.border_style(Style::default().fg(app.config.theme.accent_color()));
    }

    frame.render_widget(
        List::new(tree_items)
            .block(tree_block)
            .style(Style::default().bg(app.config.theme.surface0_color())),
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

    // Show navigation info for query input
    let query_nav_info = if app.active_pane == Pane::QueryInput {
        format!(" [{}]", app.navigation_manager.get_navigation_info())
    } else {
        String::new()
    };

    // WHERE clause
    let where_title = format!("WHERE{}", query_nav_info);
    let mut where_block = Block::default()
        .title(where_title)
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
    let where_content = if app.active_pane == Pane::QueryInput
        && app.cursor_position.0 == 0
        && app.input_mode == InputMode::Insert
    {
        // Use VimEditor content when in insert mode
        let vim_editor = app.navigation_manager.box_manager().vim_editor();
        let mut content = vim_editor.content().to_string();
        // Convert character position to byte position safely
        let byte_pos = content
            .char_indices()
            .nth(app.cursor_position.1)
            .map(|(pos, _)| pos)
            .unwrap_or(content.len());
        content.insert(byte_pos, '|'); // Add cursor
        content
    } else if let Some(state) = query_state {
        state.where_clause.clone()
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
    let order_by_content = if app.active_pane == Pane::QueryInput
        && app.cursor_position.0 == 1
        && app.input_mode == InputMode::Insert
    {
        // Use VimEditor content when in insert mode
        let vim_editor = app.navigation_manager.box_manager().vim_editor();
        let mut content = vim_editor.content().to_string();
        // Convert character position to byte position safely
        let byte_pos = content
            .char_indices()
            .nth(app.cursor_position.1)
            .map(|(pos, _)| pos)
            .unwrap_or(content.len());
        content.insert(byte_pos, '|'); // Add cursor
        content
    } else if let Some(state) = query_state {
        state.order_by_clause.clone()
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

    // Calculate available width for tabs (accounting for borders and dividers)
    let available_width = area.width.saturating_sub(4); // Account for borders
    let tab_count = app.result_tabs.len();
    let divider_width = 3; // " | " = 3 characters
    let total_divider_width = if tab_count > 1 {
        (tab_count - 1) * divider_width
    } else {
        0
    };
    let max_tab_width = if tab_count > 0 {
        (available_width.saturating_sub(total_divider_width as u16) / tab_count as u16).max(8)
    } else {
        8
    };

    // Create tab titles with color coding
    let tab_titles: Vec<Line> = app
        .result_tabs
        .iter()
        .enumerate()
        .map(|(index, (name, _, _))| {
            let shortened_name =
                shorten_tab_name_intelligent(name, &app.result_tabs, max_tab_width as usize);
            let color = get_tab_color(name, index);
            Line::from(Span::styled(shortened_name, Style::default().fg(color)))
        })
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
    let results_nav_info = if app.active_pane == Pane::Results {
        format!(" [{}]", app.navigation_manager.get_navigation_info())
    } else {
        String::new()
    };

    let results_title = format!("Results{}", results_nav_info);
    let mut block = Block::default()
        .title(results_title)
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

/// Renders a floating command window centered in the middle of the screen
fn render_floating_command_window(frame: &mut Frame, app: &App) {
    // Create a floating window centered in the middle of the screen
    let window_height = 3; // Fixed height for command input only
    let window_width = 60;

    // Center the window both horizontally and vertically
    let command_area = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Percentage((100 - (window_height * 100 / frame.area().height)) / 2),
            Constraint::Length(window_height),
            Constraint::Percentage((100 - (window_height * 100 / frame.area().height)) / 2),
        ])
        .split(frame.area())[1];

    let command_area = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage((100 - window_width) / 2),
            Constraint::Percentage(window_width),
            Constraint::Percentage((100 - window_width) / 2),
        ])
        .split(command_area)[1];

    // Clear the area and render the command window
    frame.render_widget(Clear, command_area);

    let command_text = format!(">{}", app.command_input);

    // Add preview indicator if we have suggestions and a selection
    let display_text = if app.selected_suggestion.is_some() && !app.command_suggestions.is_empty() {
        format!("{} [PREVIEW]", command_text)
    } else {
        command_text
    };

    // Create the command window block
    let command_block = Block::default()
        .title("Command")
        .borders(Borders::ALL)
        .style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface1_color()),
        );

    frame.render_widget(command_block.clone(), command_area);

    // Get inner area for content
    let inner_area = command_block.inner(command_area);

    // Render command input
    frame.render_widget(
        Paragraph::new(display_text).style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface1_color()),
        ),
        inner_area,
    );

    // Place cursor at the end of the command input (after the > prompt)
    let cursor_x = inner_area.x + 1 + app.command_input.len() as u16;
    let cursor_y = inner_area.y;
    frame.set_cursor_position(ratatui::layout::Position {
        x: cursor_x,
        y: cursor_y,
    });

    // Render suggestions dropdown if there are suggestions and meaningful input
    if !app.command_suggestions.is_empty() && !app.command_input.is_empty() {
        render_suggestions_dropdown(frame, app, command_area);
    }
}

/// Renders a minimal suggestions dropdown below the command pane
fn render_suggestions_dropdown(frame: &mut Frame, app: &App, command_area: Rect) {
    // Calculate dropdown position below the command window
    let dropdown_height = std::cmp::min(app.command_suggestions.len() as u16, 6); // Max 6 lines

    // Position dropdown directly below the command window
    let dropdown_y = command_area.y + command_area.height;

    // Align suggestions with command text - offset by the "> " prompt
    let command_text_offset = 2; // "> " is 2 characters
    let dropdown_x = command_area.x + command_text_offset;
    let dropdown_width = command_area.width.saturating_sub(command_text_offset);

    // Ensure dropdown doesn't go off screen
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

    // Clear the dropdown area
    frame.render_widget(Clear, dropdown_area);

    // Render suggestions list with scrolling
    const VISIBLE_ITEMS: usize = 6;
    let total_items = app.command_suggestions.len();
    let scroll_offset = app.suggestions_scroll_offset;

    // Get visible suggestions based on scroll offset
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

    // Render scrollbar if needed
    if total_items > VISIBLE_ITEMS {
        render_scrollbar(frame, app, dropdown_area, total_items, VISIBLE_ITEMS);
    }
}

/// Renders a scrollbar for the suggestions dropdown
fn render_scrollbar(
    frame: &mut Frame,
    app: &App,
    dropdown_area: Rect,
    total_items: usize,
    visible_items: usize,
) {
    if total_items <= visible_items {
        return;
    }

    // Calculate scrollbar position and size
    let scrollbar_width = 1;
    let scrollbar_x = dropdown_area.x + dropdown_area.width - scrollbar_width;
    let scrollbar_area = Rect {
        x: scrollbar_x,
        y: dropdown_area.y,
        width: scrollbar_width,
        height: dropdown_area.height,
    };

    // Calculate thumb position and size
    let thumb_height =
        ((visible_items as f32 / total_items as f32) * dropdown_area.height as f32) as u16;
    let thumb_height = thumb_height.max(1);

    let scroll_progress =
        app.suggestions_scroll_offset as f32 / (total_items - visible_items) as f32;
    let thumb_y =
        dropdown_area.y + (scroll_progress * (dropdown_area.height - thumb_height) as f32) as u16;

    // Render scrollbar track
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

    // Render scrollbar thumb
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

/// Intelligently shortens tab names to fit within the available width.
/// Analyzes all open tabs to determine what distinguishing information to preserve.
pub fn shorten_tab_name_intelligent(
    full_name: &str,
    all_tabs: &[(String, crate::database::QueryResult, crate::app::QueryState)],
    max_width: usize,
) -> String {
    if full_name.len() <= max_width {
        return full_name.to_string();
    }

    // Parse the full name: "connection:database:schema.table" or "connection:schema.table"
    let parts: Vec<&str> = full_name.split(':').collect();

    // Local helpers removed to avoid lifetime issues; inline splits below

    let others = all_tabs
        .iter()
        .map(|(n, _, _)| n.as_str())
        .filter(|&n| n != full_name)
        .collect::<Vec<_>>();

    if parts.len() == 2 {
        let connection = parts[0];
        let schema_table = parts[1];
        let (schema, table) = if let Some(dot_pos) = schema_table.rfind('.') {
            (&schema_table[..dot_pos], &schema_table[dot_pos + 1..])
        } else {
            ("", schema_table)
        };

        let needs_connection = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            ps.len() == 2 && ps[1] == schema_table
        });

        let needs_schema = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            match ps.len() {
                2 => ps[1]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[1][..p];
                        let otable = &ps[1][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                3 => ps[2]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[2][..p];
                        let otable = &ps[2][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                _ => false,
            }
        });

        let mut candidates: Vec<String> = Vec::new();
        if !needs_connection && !needs_schema {
            if table.len() <= max_width {
                return table.to_string();
            }
        }
        if needs_schema && schema_table.len() <= max_width {
            return schema_table.to_string();
        }

        let conn_abbrev = abbreviate_name(connection, 3);
        candidates.push(format!("{}:{}", conn_abbrev, schema_table));
        candidates.push(format!("{}:{}", conn_abbrev, table));

        if let Some(best) = candidates.into_iter().find(|c| c.len() <= max_width) {
            return best;
        }

        if max_width > 3 {
            return format!("{}...", &schema_table[..max_width.saturating_sub(3)]);
        }
    } else if parts.len() == 3 {
        let connection = parts[0];
        let database = parts[1];
        let schema_table = parts[2];
        let (schema, table) = if let Some(dot_pos) = schema_table.rfind('.') {
            (&schema_table[..dot_pos], &schema_table[dot_pos + 1..])
        } else {
            ("", schema_table)
        };

        let needs_connection = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            ps.len() == 3 && ps[1] == database && ps[2] == schema_table
        });
        let needs_database = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            (ps.len() == 2 && ps[1] == schema_table)
                || (ps.len() == 3 && ps[2] == schema_table && ps[1] != database)
        });
        let needs_schema = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            match ps.len() {
                3 => ps[2]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[2][..p];
                        let otable = &ps[2][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                2 => ps[1]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[1][..p];
                        let otable = &ps[1][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                _ => false,
            }
        });
        let has_different_schemas = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            if ps.len() == 3 && ps[0] == connection && ps[1] == database {
                ps[2]
                    .rfind('.')
                    .map(|p| &ps[2][..p] != schema)
                    .unwrap_or(false)
            } else {
                false
            }
        });
        let has_multiple_databases_same_schema = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            if ps.len() == 3 && ps[0] == connection && ps[1] != database {
                ps[2]
                    .rfind('.')
                    .map(|p| &ps[2][..p] == schema)
                    .unwrap_or(false)
            } else {
                false
            }
        });

        let mut candidates: Vec<String> = Vec::new();

        if has_multiple_databases_same_schema {
            candidates.push(format!("{}.{}", database, table));
            let db3 = abbreviate_name(database, 3);
            candidates.push(format!("{}.{}", db3, table));
            let sep = 1usize;
            if max_width > db3.len() + sep {
                let remain = max_width - db3.len() - sep;
                if remain > 1 {
                    let tfit = if table.len() > remain {
                        abbreviate_name(table, remain)
                    } else {
                        table.to_string()
                    };
                    candidates.push(format!("{}.{}", db3, tfit));
                }
            }
        }
        if has_different_schemas {
            candidates.push(schema_table.to_string());
            candidates.push(format!("{}.{}", abbreviate_name(schema, 3), table));
        }
        if !needs_connection && !needs_database && !needs_schema && !has_different_schemas {
            candidates.push(table.to_string());
        }
        if !needs_connection && needs_database {
            candidates.push(format!("{}.{}", database, table));
            candidates.push(format!("{}:{}", database, schema_table));
            candidates.push(format!("{}.{}", abbreviate_name(database, 3), table));
        }
        if needs_connection && !needs_database {
            let c3 = abbreviate_name(connection, 3);
            candidates.push(format!("{}:{}", c3, table));
            candidates.push(format!("{}:{}", c3, schema_table));
        }
        if needs_connection && needs_database {
            let c2 = abbreviate_name(connection, 2);
            let d3 = abbreviate_name(database, 3);
            candidates.push(format!("{}:{}:{}", c2, d3, schema_table));
            candidates.push(format!("{}:{}.{}", c2, d3, table));
        }

        // Generic abbreviated options
        let c2 = abbreviate_name(connection, 2);
        let d3 = abbreviate_name(database, 3);
        candidates.push(format!("{}:{}:{}", c2, d3, schema_table));
        candidates.push(format!("{}:{}.{}", c2, d3, table));
        candidates.push(format!("{}.{}", d3, table));

        if let Some(best) = candidates.into_iter().find(|c| c.len() <= max_width) {
            return best;
        }

        if has_multiple_databases_same_schema {
            let full = format!("{}.{}", database, table);
            if full.len() <= max_width {
                return full;
            }
            let d3 = abbreviate_name(database, 3);
            let d3_full = format!("{}.{}", d3, table);
            if d3_full.len() <= max_width {
                return d3_full;
            }
            if max_width > 2 {
                for dbl in (2..=3).rev() {
                    let dab = abbreviate_name(database, dbl);
                    if max_width > dab.len() + 1 {
                        let rem = max_width - dab.len() - 1;
                        let tab = if table.len() > rem {
                            abbreviate_name(table, rem)
                        } else {
                            table.to_string()
                        };
                        let cand = format!("{}.{}", dab, tab);
                        if cand.len() <= max_width {
                            return cand;
                        }
                    }
                }
                let db_only = abbreviate_name(database, max_width.min(3));
                if !db_only.is_empty() {
                    return db_only;
                }
            }
        }

        // As a last pass, try abbreviated conn/db variants
        let c2 = abbreviate_name(connection, 2);
        let d3 = abbreviate_name(database, 3);
        let full = format!("{}:{}:{}", c2, d3, schema_table);
        if full.len() <= max_width {
            return full;
        }
        if let Some(p) = schema_table.rfind('.') {
            let t = &schema_table[p + 1..];
            let conn_db_t = format!("{}:{}.{}", c2, d3, t);
            if conn_db_t.len() <= max_width {
                return conn_db_t;
            }
            let db_t = format!("{}.{}", d3, t);
            if db_t.len() <= max_width {
                return db_t;
            }
            let c_t = format!("{}:{}", c2, t);
            if c_t.len() <= max_width {
                return c_t;
            }
        }

        if max_width > 3 {
            return format!("{}...", &schema_table[..max_width.saturating_sub(3)]);
        }
    }

    if max_width > 3 {
        format!("{}...", &full_name[..max_width.saturating_sub(3)])
    } else {
        "..".to_string()
    }
}

/// Abbreviates a name to the specified length, taking characters from the beginning and end
pub fn abbreviate_name(name: &str, target_length: usize) -> String {
    if name.len() <= target_length {
        return name.to_string();
    }

    if target_length <= 2 {
        return name[..target_length].to_string();
    }

    // Take first and last characters with ellipsis in between
    let first_chars = (target_length + 1) / 2;
    let last_chars = target_length - first_chars - 1;

    if first_chars + last_chars >= name.len() {
        return name.to_string();
    }

    format!(
        "{}.{}",
        &name[..first_chars],
        &name[name.len() - last_chars..]
    )
}

/// Gets a color for a tab based on its connection and database
fn get_tab_color(tab_name: &str, _index: usize) -> Color {
    // Parse the tab name to extract connection and database info
    let parts: Vec<&str> = tab_name.split(':').collect();

    let connection = parts.get(0).unwrap_or(&"");
    let database = parts.get(1).unwrap_or(&"");

    // Create a hash from connection and database for consistent coloring
    let mut hash = 0u32;
    for byte in connection.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    for byte in database.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }

    // Use the hash to select from a palette of distinguishable colors
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Yellow,
        Color::Magenta,
        Color::Cyan,
        Color::LightRed,
        Color::LightGreen,
        Color::LightBlue,
        Color::LightYellow,
        Color::LightMagenta,
        Color::LightCyan,
    ];

    colors[hash as usize % colors.len()]
}
