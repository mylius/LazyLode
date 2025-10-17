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
    cursor::SetCursorStyle,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use input::TreeAction;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::cmp::{max, min};
use std::io;
use std::panic;

use ratatui::layout::Direction as LayoutDirection;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, Borders};

use crate::app::{ActiveBlock, App, ConnectionForm, InputMode};
use crate::input::{Action, KeyConfig, NavigationAction};
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

fn matches_search_key(keymap: &KeyConfig, code: KeyCode) -> bool {
    match code {
        KeyCode::Char(c) => c == keymap.search_key,
        _ => false,
    }
}

async fn run_app_tick<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    terminal.draw(|f| ui::render(f, app))?;

    // Update terminal cursor style based on input mode
    let cursor_style = match app.input_mode {
        InputMode::Normal => SetCursorStyle::SteadyBlock,
        _ => SetCursorStyle::SteadyBar,
    };
    let _ = execute!(io::stdout(), cursor_style);

    match event::read()? {
        Event::Key(key) => {
            if KeyCode::Char('q') == key.code && key.modifiers.is_empty() {
                app.quit();
            }
            if app.input_mode == InputMode::Normal
                && key.modifiers.is_empty()
                && matches_search_key(&app.config.keymap, key.code)
            {
                if !app.show_connection_modal {
                    app.focus_where_input();
                }
                return Ok(());
            }
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
                        }
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
            Action::Navigation(NavigationAction::FocusPane(pane)) => {
                app.active_pane = pane;
                if pane == Pane::QueryInput {
                    let len = app.get_current_field_length();
                    app.cursor_position.1 = app.cursor_position.1.min(len);
                }
            }
            Action::Navigation(nav_action) => {
                app.handle_navigation(nav_action);
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
        }
        KeyCode::Enter => {
            let command = app.command_input.clone();
            app.command_input.clear();
            app.input_mode = InputMode::Normal;
            app.active_block = ActiveBlock::Connections;
            
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
        KeyCode::Backspace => {
            app.command_input.pop();
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
        }
        _ => {}
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

    let query_chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(query_area);
    let where_inner = Block::default()
        .borders(Borders::ALL)
        .inner(query_chunks[0]);
    let order_by_inner = Block::default()
        .borders(Borders::ALL)
        .inner(query_chunks[1]);

    let results_inner = Block::default().borders(Borders::ALL).inner(results_area);
    let pagination_inner = Block::default()
        .borders(Borders::ALL)
        .inner(pagination_area);

    let x = me.column;
    let y = me.row;

    match me.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Sidebar: connections list
            if x >= conn_list_inner.x
                && x < conn_list_inner.x + conn_list_inner.width
                && y >= conn_list_inner.y
                && y < conn_list_inner.y + conn_list_inner.height
            {
                app.active_pane = Pane::Connections;
                let rel_y = y.saturating_sub(conn_list_inner.y);
                let total_items = app.get_total_visible_items() as u16;
                if total_items > 0 {
                    let idx = min(rel_y, total_items - 1) as usize;
                    if app.selected_connection_idx == Some(idx) {
                        if let Err(e) = app.toggle_tree_item(idx).await {
                            let _ = logging::error(&format!(
                                "Error toggling tree item at {}: {}",
                                idx, e
                            ));
                        }
                    } else {
                        app.selected_connection_idx = Some(idx);
                    }
                }
                return Ok(());
            }

            // Query input: WHERE block
            if x >= where_inner.x
                && x < where_inner.x + where_inner.width
                && y >= where_inner.y
                && y < where_inner.y + where_inner.height
            {
                app.active_pane = Pane::QueryInput;
                app.input_mode = InputMode::Insert;
                app.cursor_position.0 = 0;
                let rel_x = x.saturating_sub(where_inner.x) as usize;
                let len = app.get_current_field_length();
                app.cursor_position.1 = min(rel_x, len);
                return Ok(());
            }

            // Query input: ORDER BY block
            if x >= order_by_inner.x
                && x < order_by_inner.x + order_by_inner.width
                && y >= order_by_inner.y
                && y < order_by_inner.y + order_by_inner.height
            {
                app.active_pane = Pane::QueryInput;
                app.input_mode = InputMode::Insert;
                app.cursor_position.0 = 1;
                let rel_x = x.saturating_sub(order_by_inner.x) as usize;
                let len = app.get_current_field_length();
                app.cursor_position.1 = min(rel_x, len);
                return Ok(());
            }

            if let Some(tabs_area) = tabs_area {
                if x >= tabs_area.x
                    && x < tabs_area.x + tabs_area.width
                    && y >= tabs_area.y
                    && y < tabs_area.y + tabs_area.height
                {
                    if let Some(tab_count) = Some(app.result_tabs.len()).filter(|c| *c > 0) {
                        let seg_w = max(1, tabs_area.width / tab_count as u16);
                        let rel_x = x.saturating_sub(tabs_area.x);
                        let idx = min((rel_x / seg_w) as usize, tab_count - 1);
                        app.selected_result_tab_index = Some(idx);
                    }
                    return Ok(());
                }
            }

            // Results table: select row/column approximately
            if x >= results_inner.x
                && x < results_inner.x + results_inner.width
                && y >= results_inner.y
                && y < results_inner.y + results_inner.height
            {
                app.active_pane = Pane::Results;
                if let Some((_, result, _)) = app
                    .selected_result_tab_index
                    .and_then(|idx| app.result_tabs.get(idx))
                {
                    if !result.rows.is_empty() {
                        let header_rows = 1u16;
                        if y >= results_inner.y + header_rows {
                            let rel_y = y - results_inner.y - header_rows;
                            let row_idx = min(rel_y as usize, result.rows.len() - 1);
                            app.cursor_position.1 = row_idx;
                        }

                        // Column mapping using exact remainder distribution and spacing
                        let data_cols = result.columns.len();
                        if data_cols > 0 {
                            let max_lines = result.rows.len();
                            let line_num_width =
                                max(3usize, max_lines.to_string().len()) as u16 + 1;
                            if x >= results_inner.x + line_num_width {
                                let dc = data_cols as u16;
                                let spacing: u16 = 1;
                                let total_spacing = spacing.saturating_mul(dc.saturating_sub(1));
                                let remaining_w = results_inner
                                    .width
                                    .saturating_sub(line_num_width)
                                    .saturating_sub(total_spacing);
                                let base = if dc > 0 { remaining_w / dc } else { 0 };
                                let rem = if dc > 0 { remaining_w % dc } else { 0 };
                                let rel_x = x - results_inner.x - line_num_width;

                                // Walk through buckets: rem left-most buckets have width base+1, others base
                                let mut acc: u16 = 0;
                                let mut idx: usize = 0;
                                for i in 0..dc {
                                    let w = base + if i < rem { 1 } else { 0 };
                                    let next_acc = acc + w;
                                    if rel_x < next_acc {
                                        idx = i as usize;
                                        break;
                                    }
                                    // add spacing after each column except last
                                    acc = next_acc + if i + 1 < dc { spacing } else { 0 };
                                    if i + 1 == dc {
                                        idx = i as usize;
                                    }
                                }
                                app.cursor_position.0 = min(idx, data_cols - 1);
                            }
                        }
                    }
                }
                return Ok(());
            }

            // Pagination block: focus results pane
            if x >= pagination_inner.x
                && x < pagination_inner.x + pagination_inner.width
                && y >= pagination_inner.y
                && y < pagination_inner.y + pagination_inner.height
            {
                app.active_pane = Pane::Results;
                return Ok(());
            }
        }
        MouseEventKind::ScrollUp => {
            match app.active_pane {
                Pane::Results => {
                    app.move_cursor_in_results(crate::ui::types::Direction::Up);
                    return Ok(());
                }
                Pane::Connections => {
                    app.move_selection_up();
                    return Ok(());
                }
                _ => {}
            }
            // Fallback to hover-based behavior
            if y >= results_inner.y && y < results_inner.y + results_inner.height {
                app.active_pane = Pane::Results;
                app.move_cursor_in_results(crate::ui::types::Direction::Up);
                return Ok(());
            }
            if y >= pagination_inner.y && y < pagination_inner.y + pagination_inner.height {
                app.active_pane = Pane::Results;
                return Ok(());
            }
            if y >= conn_list_inner.y && y < conn_list_inner.y + conn_list_inner.height {
                app.active_pane = Pane::Connections;
                app.move_selection_up();
                return Ok(());
            }
        }
        MouseEventKind::ScrollDown => {
            match app.active_pane {
                Pane::Results => {
                    app.move_cursor_in_results(crate::ui::types::Direction::Down);
                    return Ok(());
                }
                Pane::Connections => {
                    app.move_selection_down();
                    return Ok(());
                }
                _ => {}
            }
            // Fallback to hover-based behavior
            if y >= results_inner.y && y < results_inner.y + results_inner.height {
                app.active_pane = Pane::Results;
                app.move_cursor_in_results(crate::ui::types::Direction::Down);
                return Ok(());
            }
            if y >= pagination_inner.y && y < pagination_inner.y + pagination_inner.height {
                app.active_pane = Pane::Results;
                return Ok(());
            }
            if y >= conn_list_inner.y && y < conn_list_inner.y + conn_list_inner.height {
                app.active_pane = Pane::Connections;
                app.move_selection_down();
                return Ok(());
            }
        }
        _ => {}
    }

    Ok(())
}
