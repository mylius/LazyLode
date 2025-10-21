mod app;
mod command;
mod config;
mod database;
mod input;
mod logging;
mod navigation;
mod theme;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::executor;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::cmp::{max, min};
use std::io;
use std::panic;

use ratatui::layout::Direction as LayoutDirection;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::widgets::{Block, Borders};

use crate::app::{App, InputMode};
use crate::navigation::types::NavigationAction;
use crate::navigation::NavigationInputHandler;
use crate::ui::types::Pane;

fn setup_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let panic_message = if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
            msg.to_string()
        } else if let Some(msg) = panic_info.payload().downcast_ref::<String>() {
            msg.clone()
        } else {
            "Unknown panic message".to_string()
        };

        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        if let Err(e) = logging::error(&format!(
            "PANIC:\nMessage: {}\nLocation: {}",
            panic_message, location
        )) {
            eprintln!("Failed to log panic: {}", e);
        }

        // Also log the backtrace if RUST_BACKTRACE is enabled
        if std::env::var("RUST_BACKTRACE").unwrap_or_default() == "1" {
            if let Err(e) = logging::error(&format!(
                "Backtrace:\n{:?}",
                std::backtrace::Backtrace::capture()
            )) {
                eprintln!("Failed to log backtrace: {}", e);
            }
        }
    }));
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    logging::init_logger().context("Failed to initialize logger")?;

    setup_panic_hook();
    logging::info("Starting LazyLode Database Explorer")?;

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    logging::debug("Enabled raw mode")?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    logging::debug("Entered alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;
    logging::debug("Created terminal")?;

    let app = App::new_with_async_connections()
        .await
        .context("Failed to initialize application with async connections")?;
    logging::debug("Initialized application with async connections")?;

    let res = run_app(&mut terminal, app).await; // Note: now awaiting run_app

    // Cleanup and restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    if let Err(err) = res {
        logging::error(&format!("Application error: {}", err))?;
        return Err(anyhow::anyhow!(err));
    }

    logging::info("Application terminated successfully")?;
    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        run_app_tick(terminal, &mut app).await?;

        if app.should_quit {
            return Ok(());
        }
    }
}

async fn run_app_tick<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    // Check for completed background prefetching
    if let Err(e) = app.check_background_prefetching() {
        let _ = logging::error(&format!("Error checking background prefetching: {}", e));
    }

    terminal.draw(|f| ui::render(f, app))?;

    // Update terminal cursor style based on input mode
    let cursor_style = match app.input_mode {
        InputMode::Normal => SetCursorStyle::SteadyBlock,
        _ => SetCursorStyle::SteadyBar,
    };
    let _ = execute!(io::stdout(), cursor_style);

    match event::read()? {
        Event::Key(key) => {
            // Use the new navigation input handler
            if let Err(e) = NavigationInputHandler::handle_key(key.code, key.modifiers, app).await {
                let _ = logging::error(&format!("Error handling key input: {}", e));
            }
        }
        Event::Mouse(me) => {
            handle_mouse_event(app, me).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_connection_modal_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_connection_modal_input_normal_mode(key, app).await,
        InputMode::Insert => handle_connection_modal_input_insert_mode(key, app).await,
        _ => Ok(()), // Noop for other modes
    }
}

async fn handle_connection_modal_input_normal_mode(
    key: KeyEvent,
    app: &mut App,
) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Char('i') => {
            app.input_mode = InputMode::Insert;
        }
        KeyCode::Esc => {
            app.toggle_connection_modal();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.connection_form.current_field = (app.connection_form.current_field + 1) % 7;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.connection_form.current_field = (app.connection_form.current_field + 6) % 7;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_connection_modal_input_insert_mode(
    key: KeyEvent,
    app: &mut App,
) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.save_connection();
            app.toggle_connection_modal();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Down | KeyCode::Up => match key.code {
            KeyCode::Down => {
                app.connection_form.current_field = (app.connection_form.current_field + 1) % 7;
            }
            KeyCode::Up => {
                app.connection_form.current_field = (app.connection_form.current_field + 6) % 7;
            }
            _ => {}
        },
        _ => {
            // Handle text input
            match app.connection_form.current_field {
                0 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.name.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.name.pop();
                    }
                }
                1 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.host.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.host.pop();
                    }
                }
                2 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.port.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.port.pop();
                    }
                }
                3 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.username.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.username.pop();
                    }
                }
                4 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.password.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.password.pop();
                    }
                }
                5 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.database.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.database.pop();
                    }
                }
                6 => {
                    if let KeyCode::Char(c) = key.code {
                        app.connection_form.ssh_host.push(c);
                    } else if key.code == KeyCode::Backspace {
                        app.connection_form.ssh_host.pop();
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

async fn handle_connections_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_connections_input_normal_mode(key, app).await,
        InputMode::Insert => handle_connections_input_insert_mode(key, app).await,
        _ => Ok(()), // Noop for other modes
    }
}

async fn handle_connections_input_normal_mode(
    key: KeyEvent,
    app: &mut App,
) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Char('i') => {
            app.input_mode = InputMode::Insert;
        }
        KeyCode::Char('c') => {
            app.toggle_connection_modal();
        }
        KeyCode::Char('d') => {
            app.delete_connection();
        }
        KeyCode::Char('e') => {
            app.edit_connection();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(max_idx) = app.connection_tree.len().checked_sub(1) {
                app.selected_connection_idx =
                    Some((app.selected_connection_idx.unwrap_or(0) + 1).min(max_idx));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(current_idx) = app.selected_connection_idx {
                app.selected_connection_idx = Some(current_idx.saturating_sub(1));
            }
        }
        KeyCode::Enter => {
            if let Some(idx) = app.selected_connection_idx {
                app.connect_to_database(idx);
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_connections_input_insert_mode(
    key: KeyEvent,
    app: &mut App,
) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_query_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_query_input_normal_mode(key, app).await,
        InputMode::Insert => handle_query_input_insert_mode(key, app).await,
        _ => Ok(()), // Noop for other modes
    }
}

async fn handle_query_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Char('i') => {
            app.input_mode = InputMode::Insert;
        }
        KeyCode::Char('r') => {
            app.run_query();
        }
        KeyCode::Char('c') => {
            app.clear_query();
        }
        KeyCode::Char('s') => {
            app.save_query();
        }
        KeyCode::Char('l') => {
            app.load_query();
        }
        KeyCode::Char('h') => {
            app.show_help();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_query_input_insert_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.run_query();
            app.input_mode = InputMode::Normal;
        }
        _ => {
            // Handle text input
            if let KeyCode::Char(c) = key.code {
                app.query.push(c);
            } else if key.code == KeyCode::Backspace {
                app.query.pop();
            }
        }
    }
    Ok(())
}

async fn handle_results_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_results_input_normal_mode(key, app).await,
        InputMode::Insert => handle_results_input_insert_mode(key, app).await,
        _ => Ok(()), // Noop for other modes
    }
}

async fn handle_results_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Char('i') => {
            app.input_mode = InputMode::Insert;
        }
        KeyCode::Char('c') => {
            app.copy_cell();
        }
        KeyCode::Char('d') => {
            app.delete_selected_rows();
        }
        KeyCode::Char('u') => {
            app.undo_deletion();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_cursor_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_cursor_up();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.move_cursor_left();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.move_cursor_right();
        }
        KeyCode::PageDown => {
            app.page_down();
        }
        KeyCode::PageUp => {
            app.page_up();
        }
        KeyCode::Home => {
            app.move_cursor_to_start();
        }
        KeyCode::End => {
            app.move_cursor_to_end();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_results_input_insert_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_command_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.command_buffer.clear();
        }
        KeyCode::Enter => {
            app.execute_command();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.command_buffer.pop();
        }
        KeyCode::Up => {
            app.command_history_up();
        }
        KeyCode::Down => {
            app.command_history_down();
        }
        KeyCode::Tab => {
            app.cycle_suggestions();
        }
        _ => {
            if let KeyCode::Char(c) = key.code {
                app.command_buffer.push(c);
            }
        }
    }
    Ok(())
}

async fn handle_mouse_event(app: &mut App, me: MouseEvent) -> io::Result<()> {
    let (cols, rows) = crossterm::terminal::size()?;
    let root = Rect::new(0, 0, cols, rows);

    let v_chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(root);
    let main_area = v_chunks[1];

    let h_chunks = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(main_area);
    let sidebar_area = h_chunks[0];
    let main_panel_area = h_chunks[1];

    let sidebar_chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(sidebar_area);
    let conn_list_area = sidebar_chunks[1];
    let conn_list_inner = Block::default().borders(Borders::ALL).inner(conn_list_area);

    let main_panel_chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(1)])
        .split(main_panel_area);
    let query_area = main_panel_chunks[0];
    let results_panel_area = main_panel_chunks[1];

    let result_panel_chunks = if app.result_tabs.is_empty() {
        Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(results_panel_area)
    } else {
        Layout::default()
            .direction(LayoutDirection::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(results_panel_area)
    };

    let (tabs_area, results_area, pagination_area) = if app.result_tabs.is_empty() {
        (None, result_panel_chunks[0], result_panel_chunks[1])
    } else {
        (
            Some(result_panel_chunks[0]),
            result_panel_chunks[1],
            result_panel_chunks[2],
        )
    };

    match me.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let (x, y) = (me.column, me.row);

            // Check if click is in connection list area
            if conn_list_inner.contains(Position::new(x, y)) {
                let relative_y = y - conn_list_inner.y;
                let item_index = relative_y as usize;

                // Calculate the visual index properly by counting visible items
                let mut current_visual_index = 0;
                let mut found_index = None;

                for (conn_idx, connection) in app.connection_tree.iter().enumerate() {
                    if current_visual_index == item_index {
                        found_index = Some(current_visual_index);
                        break;
                    }
                    current_visual_index += 1;

                    if connection.is_expanded {
                        for (_db_idx, database) in connection.databases.iter().enumerate() {
                            if current_visual_index == item_index {
                                found_index = Some(current_visual_index);
                                break;
                            }
                            current_visual_index += 1;

                            if database.is_expanded {
                                for (_schema_idx, schema) in database.schemas.iter().enumerate() {
                                    if current_visual_index == item_index {
                                        found_index = Some(current_visual_index);
                                        break;
                                    }
                                    current_visual_index += 1;

                                    if schema.is_expanded {
                                        for _table in &schema.tables {
                                            if current_visual_index == item_index {
                                                found_index = Some(current_visual_index);
                                                break;
                                            }
                                            current_visual_index += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(visual_index) = found_index {
                    app.selected_connection_idx = Some(visual_index);
                    app.active_pane = Pane::Connections;
                    app.input_mode = InputMode::Normal;
                    app.last_key_was_d = false;
                    app.awaiting_replace = false;

                    // Sync with navigation manager
                    app.navigation_manager
                        .handle_action(NavigationAction::FocusConnections);

                    // Also trigger the tree action to open/expand the item
                    if let Err(e) =
                        executor::block_on(app.handle_tree_action(crate::input::TreeAction::Expand))
                    {
                        let _ = logging::error(&format!("Error expanding tree item: {}", e));
                    }
                }
            }

            // Check if click is in query area
            if query_area.contains(Position::new(x, y)) {
                app.active_pane = Pane::QueryInput;
                app.input_mode = InputMode::Insert;
                app.last_key_was_d = false;
                app.awaiting_replace = false;

                // Determine which query field was clicked based on position
                let relative_y = y - query_area.y;
                let relative_x = x - query_area.x;

                // Split query area into WHERE and ORDER BY sections
                let query_height = query_area.height;
                let where_height = query_height / 2;
                let order_by_height = query_height - where_height;

                if relative_y < where_height {
                    // Clicked in WHERE clause area
                    app.cursor_position.0 = 0;
                    // Set cursor position in WHERE clause text
                    if let Some(state) = app.current_query_state_mut() {
                        let where_text_len = state.where_clause.len();
                        let cursor_x = (relative_x as usize).min(where_text_len);
                        app.cursor_position.1 = cursor_x;
                    }
                } else {
                    // Clicked in ORDER BY clause area
                    app.cursor_position.0 = 1;
                    // Set cursor position in ORDER BY clause text
                    if let Some(state) = app.current_query_state_mut() {
                        let order_by_text_len = state.order_by_clause.len();
                        let cursor_x = (relative_x as usize).min(order_by_text_len);
                        app.cursor_position.1 = cursor_x;
                    }
                }

                // Sync with navigation manager
                app.navigation_manager
                    .handle_action(NavigationAction::FocusQueryInput);
            }

            // Check if click is in results area
            if results_area.contains(Position::new(x, y)) {
                app.active_pane = Pane::Results;
                app.input_mode = InputMode::Normal;
                app.last_key_was_d = false;
                app.awaiting_replace = false;

                // Sync with navigation manager
                app.navigation_manager
                    .handle_action(NavigationAction::FocusResults);

                // Calculate cursor position based on click coordinates
                if let Some(selected_tab_index) = app.selected_result_tab_index {
                    if let Some((_, result, _)) = app.result_tabs.get(selected_tab_index) {
                        let relative_x = x - results_area.x;
                        let relative_y = y - results_area.y;

                        // Account for table structure: borders, header, and line number column
                        let table_inner_x = relative_x.saturating_sub(1); // Account for left border
                        let table_inner_y = relative_y.saturating_sub(1); // Account for top border and header

                        // Calculate line number column width
                        let max_lines = result.rows.len();
                        let line_num_width = max_lines.to_string().len().max(3) as u16;
                        let first_col_width = line_num_width + 1; // +1 for padding

                        // Check if click is in the line number column
                        if table_inner_x < first_col_width {
                            // Click is in line number column, select first data column
                            app.cursor_position.0 = 0;
                        } else {
                            // Click is in data area, calculate which data column using exact render widths
                            let data_x = table_inner_x - first_col_width;

                            let data_cols = result.columns.len() as u16;
                            if data_cols > 0 {
                                let spacing: u16 = 1;
                                let table_inner_width = results_area.width.saturating_sub(2);
                                let total_spacing =
                                    spacing.saturating_mul(data_cols.saturating_sub(1));
                                let remaining_w = table_inner_width
                                    .saturating_sub(first_col_width)
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

                                let mut accum: u16 = 0;
                                let mut selected_col: usize = 0;
                                for i in 0..(data_cols as usize) {
                                    let w: u16 = base + if (i as u16) < rem { 1 } else { 0 };
                                    let col_end = accum.saturating_add(w);
                                    if data_x < col_end {
                                        selected_col = i;
                                        break;
                                    }
                                    accum = col_end.saturating_add(spacing);
                                    if data_x < accum {
                                        selected_col = i;
                                        break;
                                    }
                                    if i == (data_cols as usize - 1) {
                                        selected_col = i;
                                    }
                                }
                                app.cursor_position.0 =
                                    selected_col.min(result.columns.len().saturating_sub(1));
                            } else {
                                app.cursor_position.0 = 0;
                            }
                        }

                        // Calculate row (account for header row and pagination)
                        let row = if table_inner_y > 0 {
                            (table_inner_y + 1) as usize
                        } else {
                            0
                        };

                        // Account for table pagination/scrolling
                        let visible_capacity =
                            results_area.height.saturating_sub(3).max(0) as usize; // Account for borders and header
                        let total_rows = result.rows.len();
                        let max_start = total_rows.saturating_sub(visible_capacity);
                        let current_cursor_row =
                            app.cursor_position.1.min(total_rows.saturating_sub(1));
                        let start_row = if visible_capacity == 0 {
                            0
                        } else {
                            current_cursor_row
                                .saturating_sub(visible_capacity / 2)
                                .min(max_start)
                        };

                        // Convert click row to actual data row index
                        let actual_row = start_row + row;
                        app.cursor_position.1 = actual_row.min(result.rows.len().saturating_sub(1));
                    }
                }
            }

            // Check if click is in tabs area
            if let Some(tabs_rect) = tabs_area {
                if tabs_rect.contains(Position::new(x, y)) {
                    let tab_count = app.result_tabs.len();
                    if tab_count > 0 {
                        // Mirror render_result_tabs width calculation
                        let available_width = tabs_rect.width.saturating_sub(4);
                        let divider_width: u16 = 3; // " | "
                        let total_divider_width = if tab_count > 1 {
                            (tab_count as u16 - 1).saturating_mul(divider_width)
                        } else {
                            0
                        };
                        let max_tab_width: u16 = if tab_count > 0 {
                            (available_width
                                .saturating_sub(total_divider_width)
                                / tab_count as u16)
                                .max(8)
                        } else {
                            8
                        };

                        // Compute displayed label widths
                        let mut label_widths: Vec<u16> = Vec::with_capacity(tab_count);
                        for (name, _, _) in app.result_tabs.iter() {
                            let shortened = crate::ui::shorten_tab_name_intelligent(
                                name,
                                &app.result_tabs,
                                max_tab_width as usize,
                            );
                            let w = shortened.len().min(max_tab_width as usize) as u16;
                            // Ensure at least 1 width to be clickable
                            let w = w.max(1);
                            label_widths.push(w);
                        }

                        // Map click x to content x (Tabs draws from left edge)
                        let content_x = x.saturating_sub(tabs_rect.x);

                        // Walk through tabs accumulating widths + dividers
                        let mut accum: u16 = 0;
                        let mut clicked_index: Option<usize> = None;
                        for i in 0..tab_count {
                            let w = label_widths[i];
                            let end = accum.saturating_add(w);
                            if content_x < end {
                                clicked_index = Some(i);
                                break;
                            }
                            accum = end.saturating_add(divider_width);
                            if content_x < accum {
                                // Click on divider selects tab to the left
                                clicked_index = Some(i);
                                break;
                            }
                        }

                        if let Some(tab_index) = clicked_index {
                            if tab_index < tab_count {
                                app.selected_result_tab_index = Some(tab_index);
                                app.cursor_position = (0, 0);
                                app.navigation_manager
                                    .handle_action(NavigationAction::FocusResults);
                            }
                        }
                    }
                }
            }

            // Check if click is in pagination area
            if pagination_area.contains(Position::new(x, y)) {
                let pagination_width = pagination_area.width;
                let button_width = pagination_width / 4;
                let button_index = ((x - pagination_area.x) / button_width) as usize;

                match button_index {
                    0 => {
                        let _ = app.first_page().await;
                    }
                    1 => {
                        let _ = app.previous_page().await;
                    }
                    2 => {
                        let _ = app.next_page().await;
                    }
                    3 => {
                        let _ = app.last_page().await;
                    }
                    _ => {}
                }
            }
        }
        MouseEventKind::ScrollUp => match app.active_pane {
            Pane::Connections => {
                if app.selected_connection_idx.is_some() {
                    let current = app.selected_connection_idx.unwrap();
                    app.selected_connection_idx = Some(max(0, current.saturating_sub(1)));
                }
            }
            Pane::Results => {
                let _ = app.previous_page().await;
            }
            _ => {}
        },
        MouseEventKind::ScrollDown => match app.active_pane {
            Pane::Connections => {
                if app.selected_connection_idx.is_some() {
                    let current = app.selected_connection_idx.unwrap();
                    let total_visible_items = app.get_total_visible_items();
                    app.selected_connection_idx =
                        Some(min(total_visible_items.saturating_sub(1), current + 1));
                }
            }
            Pane::Results => {
                let _ = app.next_page().await;
            }
            _ => {}
        },
        _ => {}
    }

    Ok(())
}
