mod app;
mod command;
mod config;
mod database;
mod input;
mod logging;
mod theme;
mod ui;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use input::TreeAction;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::panic;

use crate::app::{ActiveBlock, App, ConnectionForm, InputMode};
use crate::input::{Action, NavigationAction};
use crate::ui::types::{Direction, Pane};

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

    let app = App::new();
    logging::debug("Initialized application")?;

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
    terminal.draw(|f| ui::render(f, app))?;

    if let Event::Key(key) = event::read()? {
        if KeyCode::Char('q') == key.code && key.modifiers.is_empty() {
            app.quit();
        }
        if app.show_connection_modal {
            if let ActiveBlock::ConnectionModal = app.active_block {
                handle_connection_modal_input(key, app).await?;
            }
        } else {
            match app.active_pane {
                Pane::Connections => {
                    handle_connections_input(key, app).await?;
                }
                Pane::QueryInput => {
                    handle_query_input(key, app).await?;
                }
                Pane::Results => {
                    handle_results_input(key, app).await?;
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
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
            app.connection_form.current_field = (app.connection_form.current_field + 1) % 6;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.connection_form.current_field = (app.connection_form.current_field + 5) % 6;
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
                app.connection_form.current_field = (app.connection_form.current_field + 1) % 6;
            }
            KeyCode::Up => {
                app.connection_form.current_field = (app.connection_form.current_field + 5) % 6;
            }
            _ => {}
        },
        KeyCode::Backspace => match app.connection_form.current_field {
            0 => {
                app.connection_form.name.pop();
            }
            1 => {
                app.connection_form.host.pop();
            }
            2 => {
                app.connection_form.port.pop();
            }
            3 => {
                app.connection_form.username.pop();
            }
            4 => {
                app.connection_form.password.pop();
            }
            5 => {
                app.connection_form.database.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => {
            // For port field, only allow numeric input
            if app.connection_form.current_field == 2 {
                if c.is_ascii_digit() {
                    app.connection_form.port.push(c);
                }
            } else {
                // For other fields, allow any character
                match app.connection_form.current_field {
                    0 => app.connection_form.name.push(c),
                    1 => app.connection_form.host.push(c),
                    3 => app.connection_form.username.push(c),
                    4 => app.connection_form.password.push(c),
                    5 => app.connection_form.database.push(c),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_connections_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_connections_input_normal_mode(key, app).await,
        _ => Ok(()), // Noop for other modes
    }
}

async fn handle_connections_input_normal_mode(
    key: KeyEvent,
    app: &mut App,
) -> Result<(), io::Error> {
    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(nav_action) => match nav_action {
                NavigationAction::Direction(direction) => match direction {
                    Direction::Up => app.move_selection_up(),
                    Direction::Down => app.move_selection_down(),
                    Direction::Right => {
                        if let Err(e) = app.handle_tree_action(TreeAction::Expand).await {
                            let _ = logging::error(&format!("Error expanding connection: {}", e));
                        }
                    }
                    _ => {}
                },
                NavigationAction::FocusPane(pane) => {
                    app.active_pane = pane;
                }
                _ => {
                    app.handle_navigation(nav_action);
                }
            },
            Action::TreeAction(tree_action) => {
                if let Err(e) = app.handle_tree_action(tree_action).await {
                    let _ = logging::error(&format!("Error in tree action: {}", e));
                }
            }
            Action::Edit => {
                if let Some(index) = app.selected_connection_idx {
                    let connection = &app.saved_connections[index];
                    app.connection_form = ConnectionForm {
                        name: connection.name.clone(),
                        db_type: connection.db_type.clone(),
                        host: connection.host.clone(),
                        port: connection.port.to_string(),
                        username: connection.username.clone(),
                        password: connection.password.clone().unwrap_or_default(),
                        database: connection.database.clone(),
                        editing_index: Some(index),
                        current_field: 0,
                        ssh_enabled: connection.ssh_tunnel.is_some(),
                        ssh_host: connection.ssh_tunnel.clone().unwrap_or_default().host,
                        ssh_username: connection.ssh_tunnel.clone().unwrap_or_default().username,
                        ssh_port: connection
                            .ssh_tunnel
                            .clone()
                            .unwrap_or_default()
                            .port
                            .to_string(),
                        ssh_password: connection
                            .ssh_tunnel
                            .clone()
                            .unwrap_or_default()
                            .password
                            .unwrap_or_default(),
                        ssh_key_path: connection
                            .ssh_tunnel
                            .clone()
                            .unwrap_or_default()
                            .private_key_path
                            .unwrap_or_default(),
                    };
                    app.show_connection_modal = true;
                    app.active_block = ActiveBlock::ConnectionModal;
                    app.input_mode = InputMode::Normal;
                }
            }
            Action::Delete => {
                // Handle delete action
                app.delete_connection();
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                app.quit();
            }
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                app.show_connection_modal = true;
                app.active_block = ActiveBlock::ConnectionModal;
                app.input_mode = InputMode::Normal;
            }
            _ => {}
        }
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
    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(NavigationAction::FocusPane(pane)) => {
                app.active_pane = pane;
            }
            Action::Navigation(nav_action) => {
                app.handle_navigation(nav_action);
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('i') if key.modifiers.is_empty() => {
                app.input_mode = InputMode::Insert;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn handle_query_input_insert_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if let Err(e) = app.refresh_results().await {
                let _ = logging::error(&format!("Error refreshing results: {}", e));
            }
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) => app.insert_char(c),
        KeyCode::Backspace => app.delete_char(),
        KeyCode::Up => app.handle_navigation(NavigationAction::Direction(Direction::Up)),
        KeyCode::Down => app.handle_navigation(NavigationAction::Direction(Direction::Down)),
        KeyCode::Left => app.handle_navigation(NavigationAction::Direction(Direction::Left)),
        KeyCode::Right => app.handle_navigation(NavigationAction::Direction(Direction::Right)),
        _ => {}
    }
    Ok(())
}

async fn handle_results_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_results_input_normal_mode(key, app).await,
        _ => Ok(()), // Noop for other modes
    }
}

async fn handle_results_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    if app.show_deletion_modal {
        match key.code {
            KeyCode::Esc => {
                app.show_deletion_modal = false;
                if let Some((_, _, state)) = app
                    .selected_result_tab_index
                    .and_then(|idx| app.result_tabs.get_mut(idx))
                {
                    state.rows_marked_for_deletion.clear();
                }
            }
            KeyCode::Enter => {
                if let Err(e) = app.confirm_deletions().await {
                    let _ = logging::error(&format!("Error confirming deletions: {}", e));
                }
                app.show_deletion_modal = false;
            }
            _ => {}
        }
        return Ok(());
    }

    if key.code == KeyCode::Esc {
        app.command_buffer.clear();
        return Ok(());
    }

    // Handle key input with command buffer (non-exclusive):
    // Only early-return if a command was positively processed. Otherwise, fall through
    // to regular action handling so keys like sort/tab still work.
    if let KeyCode::Char(c) = key.code {
        if key.modifiers.is_empty() {
            app.command_buffer.push(c);
            match command::CommandProcessor::process_command(app) {
                Ok(true) => {
                    return Ok(());
                }
                Ok(false) => {
                    // fall through to action handling
                }
                Err(e) => {
                    let _ = logging::error(&format!("Error processing command: {}", e));
                    app.command_buffer.clear();
                }
            }
        } else {
            app.command_buffer.clear();
        }
    } else {
        app.command_buffer.clear();
    }

    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(nav_action) => match nav_action {
                NavigationAction::Direction(direction) => {
                    app.move_cursor_in_results(direction);
                }
                NavigationAction::FocusPane(pane) => {
                    app.active_pane = pane;
                }
                _ => {
                    app.handle_navigation(nav_action);
                }
            },
            Action::Sort => {
                if let Err(e) = app.sort_results().await {
                    let _ = logging::error(&format!("Error sorting results: {}", e));
                }
            }
            Action::Delete => {
                app.toggle_row_deletion_mark();
            }
            Action::Confirm => {
                if app.show_deletion_modal {
                    // If deletion modal is shown, treat Enter as confirmation
                    match app.confirm_deletions().await {
                        Ok(_) => {
                            app.show_deletion_modal = false;
                        }
                        Err(e) => {
                            // Keep modal open on error so user can see what failed
                            // Status message is already set in confirm_deletions
                            let _ = logging::error(&format!("Error confirming deletions: {}", e));
                        }
                    }
                } else if app
                    .selected_result_tab_index
                    .and_then(|idx| app.result_tabs.get(idx))
                    .map(|(_, _, state)| !state.rows_marked_for_deletion.is_empty())
                    .unwrap_or(false)
                {
                    // If rows are marked for deletion, show confirmation modal
                    app.show_deletion_modal = true;
                }
            }
            Action::Cancel => {
                if app.show_deletion_modal {
                    app.show_deletion_modal = false;
                    app.clear_deletion_marks();
                    app.status_message = Some("Deletion cancelled".to_string());
                }
            }
            _ => {}
        }
    }
    Ok(())
}
