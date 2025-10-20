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
        self, DisableMouseCapture, EnableMouseCapture, Event, MouseButton, MouseEvent,
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
            // Handle quit key first
            if KeyCode::Char('q') == key.code && key.modifiers.is_empty() {
                app.quit();
            }
            
            // Use the new navigation input handler
            if let Err(e) = NavigationInputHandler::handle_key(key.code, key.modifiers, app).await {
                let _ = logging::error(&format!("Error handling key input: {}", e));
            }
            
            // Handle search key focus
            if app.input_mode == InputMode::Normal
                && key.modifiers.is_empty()
                && matches_search_key(&app.config.keymap, key.code)
            {
                if !app.show_connection_modal {
                    app.focus_where_input();
                }
                return Ok(());
            }
            
            // Handle connection modal
            if app.show_connection_modal {
                if let ActiveBlock::ConnectionModal = app.active_block {
                    handle_connection_modal_input(key, app).await?;
                }
            } else {
                match app.input_mode {
                    InputMode::Command => {
                        handle_command_input(key, app).await?;
                    }
                    _ => {
                        // Navigation is handled by NavigationInputHandler above
                    }
                }
            }

            if app.should_quit {
                return Ok(());
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
            6 => {
                // clear selection to None
                app.connection_form.ssh_tunnel_name = None;
            }
            _ => {}
        },
        KeyCode::Left => {
            if app.connection_form.current_field == 6 {
                let names: Vec<String> = app
                    .config
                    .ssh_tunnels
                    .iter()
                    .map(|t| t.name.clone())
                    .collect();
                if names.is_empty() {
                    app.connection_form.ssh_tunnel_name = None;
                } else {
                    let current_idx = app
                        .connection_form
                        .ssh_tunnel_name
                        .as_ref()
                        .and_then(|n| names.iter().position(|x| x == n))
                        .unwrap_or(0);
                    let new_idx = if current_idx == 0 {
                        None
                    } else {
                        Some(current_idx - 1)
                    };
                    app.connection_form.ssh_tunnel_name = new_idx.map(|i| names[i].clone());
                }
            }
        }
        KeyCode::Right => {
            if app.connection_form.current_field == 6 {
                let names: Vec<String> = app
                    .config
                    .ssh_tunnels
                    .iter()
                    .map(|t| t.name.clone())
                    .collect();
                if names.is_empty() {
                    app.connection_form.ssh_tunnel_name = None;
                } else {
                    let maybe_idx = app
                        .connection_form
                        .ssh_tunnel_name
                        .as_ref()
                        .and_then(|n| names.iter().position(|x| x == n));
                    let new_idx = match maybe_idx {
                        None => Some(0),
                        Some(i) if i + 1 < names.len() => Some(i + 1),
                        _ => None, // wrap to None
                    };
                    app.connection_form.ssh_tunnel_name = new_idx.map(|i| names[i].clone());
                }
            }
        }
        KeyCode::Char(c) => {
            // For port field, only allow numeric input
            if app.connection_form.current_field == 2 {
                if c.is_ascii_digit() {
                    app.connection_form.port.push(c);
                }
            } else if app.connection_form.current_field != 6 {
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
            Action::FirstPage => {
                if let Err(e) = app.first_page().await {
                    let _ = logging::error(&format!("Error going to first page: {}", e));
                }
            }
            Action::PreviousPage => {
                if let Err(e) = app.previous_page().await {
                    let _ = logging::error(&format!("Error going to previous page: {}", e));
                }
            }
            Action::NextPage => {
                if let Err(e) = app.next_page().await {
                    let _ = logging::error(&format!("Error going to next page: {}", e));
                }
            }
            Action::LastPage => {
                if let Err(e) = app.last_page().await {
                    let _ = logging::error(&format!("Error going to last page: {}", e));
                }
            }
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
                        database: connection.default_database.clone().unwrap_or_default(),
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
                        ssh_tunnel_name: connection.ssh_tunnel_name.clone(),
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
            Action::EnterCommand => {
                app.input_mode = InputMode::Command;
                app.active_block = ActiveBlock::CommandInput;
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
    // First handle query-pane Vim-like keys explicitly
    match key.code {
        KeyCode::Char('i') if key.modifiers.is_empty() => {
            app.input_mode = InputMode::Insert;
            app.last_key_was_d = false;
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('a') if key.modifiers.is_empty() => {
            let max_pos = app.get_current_field_length();
            if app.cursor_position.1 < max_pos {
                app.cursor_position.1 += 1;
            }
            app.input_mode = InputMode::Insert;
            app.last_key_was_d = false;
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('h') | KeyCode::Left if key.modifiers.is_empty() => {
            app.handle_navigation(NavigationAction::Direction(Direction::Left));
            app.last_key_was_d = false;
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('l') | KeyCode::Right if key.modifiers.is_empty() => {
            app.handle_navigation(NavigationAction::Direction(Direction::Right));
            app.last_key_was_d = false;
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('k') | KeyCode::Up if key.modifiers.is_empty() => {
            app.handle_navigation(NavigationAction::Direction(Direction::Up));
            app.last_key_was_d = false;
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('j') | KeyCode::Down if key.modifiers.is_empty() => {
            app.handle_navigation(NavigationAction::Direction(Direction::Down));
            app.last_key_was_d = false;
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('d') if key.modifiers.is_empty() => {
            if app.last_key_was_d {
                app.clear_current_field();
                app.last_key_was_d = false;
            } else {
                app.delete_char_at_cursor();
                app.last_key_was_d = true;
            }
            app.awaiting_replace = false;
            return Ok(());
        }
        KeyCode::Char('r') if key.modifiers.is_empty() => {
            app.awaiting_replace = true;
            app.last_key_was_d = false;
            return Ok(());
        }
        KeyCode::Char(c) if key.modifiers.is_empty() => {
            if app.awaiting_replace {
                app.replace_char_at_cursor(c);
                app.awaiting_replace = false;
                app.last_key_was_d = false;
                return Ok(());
            }
        }
        _ => {}
    }

    // Fallback to keymap (pane switching etc.)
    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(NavigationAction::FocusPane(pane)) => {
                app.active_pane = pane;
                if pane == Pane::QueryInput {
                    // Ensure a valid cursor position when focusing query pane
                    let len = app.get_current_field_length();
                    app.cursor_position.1 = app.cursor_position.1.min(len);
                }
            }
            Action::Navigation(nav_action) => {
                app.handle_navigation(nav_action);
            }
            Action::EnterCommand => {
                app.input_mode = InputMode::Command;
                app.active_block = ActiveBlock::CommandInput;
            }
            _ => {}
        }
        app.last_key_was_d = false;
        app.awaiting_replace = false;
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
            Action::FollowForeignKey => {
                if let Err(e) = app.follow_foreign_key().await {
                    let _ = logging::error(&format!("Error following foreign key: {}", e));
                }
            }
            Action::FirstPage => {
                if let Err(e) = app.first_page().await {
                    let _ = logging::error(&format!("Error going to first page: {}", e));
                }
            }
            Action::PreviousPage => {
                if let Err(e) = app.previous_page().await {
                    let _ = logging::error(&format!("Error going to previous page: {}", e));
                }
            }
            Action::NextPage => {
                if let Err(e) = app.next_page().await {
                    let _ = logging::error(&format!("Error going to next page: {}", e));
                }
            }
            Action::LastPage => {
                if let Err(e) = app.last_page().await {
                    let _ = logging::error(&format!("Error going to last page: {}", e));
                }
            }
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
            Action::EnterCommand => {
                app.input_mode = InputMode::Command;
                app.active_block = ActiveBlock::CommandInput;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn handle_command_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.active_block = ActiveBlock::Connections;
            app.command_input.clear();
            app.command_suggestions.clear();
            app.selected_suggestion = None;
            // Restore the saved theme if we were previewing
            let _ = app.restore_theme();
        }
        KeyCode::Enter => {
            let command = app.command_input.clone();
            app.command_input.clear();
            app.input_mode = InputMode::Normal;
            app.active_block = ActiveBlock::Connections;
            app.command_suggestions.clear();
            app.selected_suggestion = None;

            // Process the command
            if command == "themes" {
                if let Err(e) = app.list_themes() {
                    let _ = logging::error(&format!("Error listing themes: {}", e));
                }
            } else if command.starts_with("theme ") {
                let theme_name = command.strip_prefix("theme ").unwrap_or("");
                if !theme_name.is_empty() {
                    if let Err(e) = app.switch_theme(theme_name) {
                        let _ = logging::error(&format!("Error switching theme: {}", e));
                    }
                }
            } else {
                app.status_message = Some(format!("Unknown command: {}", command));
            }
        }
        KeyCode::Up => {
            app.select_previous_suggestion();
            // Preview theme if suggestion is a theme command
            if let Some(suggestion) = app.get_selected_suggestion().cloned() {
                if suggestion.starts_with("theme ") {
                    let theme_name = suggestion.strip_prefix("theme ").unwrap_or("");
                    if !theme_name.is_empty() {
                        let _ = app.preview_theme(theme_name);
                    }
                }
            }
        }
        KeyCode::Down => {
            app.select_next_suggestion();
            // Preview theme if suggestion is a theme command
            if let Some(suggestion) = app.get_selected_suggestion().cloned() {
                if suggestion.starts_with("theme ") {
                    let theme_name = suggestion.strip_prefix("theme ").unwrap_or("");
                    if !theme_name.is_empty() {
                        let _ = app.preview_theme(theme_name);
                    }
                }
            }
        }
        KeyCode::Tab => {
            app.apply_selected_suggestion();
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            app.update_command_suggestions();
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
            app.update_command_suggestions();
        }
        _ => {}
    }
    Ok(())
}

fn matches_search_key(keymap: &crate::input::KeyConfig, code: KeyCode) -> bool {
    match code {
        KeyCode::Char(c) => c == keymap.search_key,
        _ => false,
    }
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
                if item_index < app.connections.len() {
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
                        Some(min(app.connections.len().saturating_sub(1), current + 1));
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
