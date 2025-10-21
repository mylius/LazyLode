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
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseButton, MouseEvent,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::cmp::{max, min};
use std::io;
use std::panic;

use ratatui::layout::Direction as LayoutDirection;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::widgets::{Block, Borders};

use crate::app::{App, InputMode};
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

async fn handle_connections_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
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
                app.selected_connection_idx = Some((app.selected_connection_idx.unwrap_or(0) + 1).min(max_idx));
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

async fn handle_connections_input_insert_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
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
        .constraints([Constraint::Length(8), Constraint::Min(1)])
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
                
                // Only allow selection of connection items (not databases, schemas, or tables)
                if item_index < app.connection_tree.len() {
                    app.selected_connection_idx = Some(item_index);
                }
            }

            // Check if click is in query area
            if query_area.contains(Position::new(x, y)) {
                app.active_pane = Pane::QueryInput;
                app.input_mode = InputMode::Insert;
            }

            // Check if click is in results area
            if results_area.contains(Position::new(x, y)) {
                app.active_pane = Pane::Results;
            }

            // Check if click is in tabs area
            if let Some(tabs_rect) = tabs_area {
                if tabs_rect.contains(Position::new(x, y)) {
                    let tab_width = tabs_rect.width / app.result_tabs.len() as u16;
                    let tab_index = ((x - tabs_rect.x) / tab_width) as usize;
                    if tab_index < app.result_tabs.len() {
                        app.selected_result_tab_index = Some(tab_index);
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
                    app.selected_connection_idx =
                        Some(min(app.config.connections.len().saturating_sub(1), current + 1));
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
