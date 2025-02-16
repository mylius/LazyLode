mod app;
mod ui;
mod theme;
mod config;
mod database;
mod logging;
mod input;

use std::io;
use anyhow::{Result, Context};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

use crate::app::{App, InputMode, ActiveBlock};
use crate::ui::types::{Pane, Direction};
use crate::input::{Action, NavigationAction, TreeAction};


#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    logging::init_logger().context("Failed to initialize logger")?;
    logging::info("Starting LazyLode Database Explorer")?;

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    logging::debug("Enabled raw mode")?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    logging::debug("Entered alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .context("Failed to create terminal")?;
    logging::debug("Created terminal")?;

    let app = App::new();
    logging::debug("Initialized application")?;

    let res = run_app(&mut terminal, app).await;  // Note: now awaiting run_app

    // Cleanup and restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    if let Err(err) = res {
        logging::error(&format!("Application error: {}", err))?;
        return Err(anyhow::anyhow!(err));
    }

    logging::info("Application terminated successfully")?;
    Ok(())
}


async fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        run_app_tick(terminal, &mut app).await?;

        if app.should_quit {
            return Ok(());
        }
    }
}

async fn run_app_tick<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    terminal.draw(|f| ui::render(f, &app))?;

    if let Event::Key(key) = event::read()? {
        if KeyCode::Char('q') == key.code && key.modifiers.is_empty() {
            app.quit();
        }
        if app.show_connection_modal {
            match app.active_block {
                ActiveBlock::ConnectionModal => {
                    handle_connection_modal_input(key, app).await?;
                },
                _ => {}
            }
        } else {
            match app.active_pane {
                Pane::Connections => {
                    handle_connections_input(key, app).await?;
                },
                Pane::QueryInput => {
                    handle_query_input(key, app).await?;
                },
                Pane::Results => {
                    handle_results_input(key, app).await?;
                },
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

async fn handle_connection_modal_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Char('i') => {
            app.input_mode = InputMode::Insert;
        },
        KeyCode::Esc => {
            app.toggle_connection_modal();
        },
        KeyCode::Down | KeyCode::Char('j') => {
            app.connection_form.current_field = (app.connection_form.current_field + 1) % 6;
        },
        KeyCode::Up | KeyCode::Char('k') => {
            app.connection_form.current_field = (app.connection_form.current_field + 5) % 6;
        },
        _ => {}
    }
    Ok(())
}

async fn handle_connection_modal_input_insert_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        },
        KeyCode::Enter => {
            app.save_connection();
            app.toggle_connection_modal();
            app.input_mode = InputMode::Normal;
        },
        KeyCode::Down | KeyCode::Up => {
            match key.code {
                KeyCode::Down => {
                    app.connection_form.current_field = (app.connection_form.current_field + 1) % 6;
                },
                KeyCode::Up => {
                    app.connection_form.current_field = (app.connection_form.current_field + 5) % 6;
                },
                _ => {}
            }
        },
        KeyCode::Backspace => {
            match app.connection_form.current_field {
                0 => { app.connection_form.name.pop(); },
                1 => { app.connection_form.host.pop(); },
                2 => { app.connection_form.port.pop(); },
                3 => { app.connection_form.username.pop(); },
                4 => { app.connection_form.password.pop(); },
                5 => { app.connection_form.database.pop(); },
                _ => {}
            }
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
        },
        _ => {}
    }
    Ok(())
}


async fn handle_connections_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_connections_input_normal_mode(key, app).await,
        _ => Ok(()) // Noop for other modes
    }
}


async fn handle_connections_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(nav_action) => {
                match nav_action {
                    NavigationAction::Direction(direction) => {
                        match direction {
                            Direction::Up => app.move_selection_up(),
                            Direction::Down => app.move_selection_down(),
                            _ => {}
                        }
                    },
                    NavigationAction::FocusPane(pane) => {
                        app.active_pane = pane;
                    }
                }
            },
            Action::TreeAction(tree_action) => {
                if let Err(e) = app.handle_tree_action(tree_action).await {
                    logging::error(&format!("Error in tree action: {}", e));
                }
            },
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                app.quit();
            },
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                app.show_connection_modal = true;
                app.active_block = ActiveBlock::ConnectionModal;
                app.input_mode = InputMode::Normal;
            },
            _ => {}
        }
    }
    Ok(())
}


async fn handle_query_input(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match app.input_mode {
        InputMode::Normal => handle_query_input_normal_mode(key, app).await,
        InputMode::Insert => handle_query_input_insert_mode(key, app).await,
        _ => Ok(()) // Noop for other modes
    }
}

async fn handle_query_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(nav_action) => {
                app.handle_navigation(nav_action);
            },
            Action::Navigation(NavigationAction::FocusPane(pane)) => {
                app.active_pane = pane;
            },
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('i') if key.modifiers.is_empty() => {
                app.input_mode = InputMode::Insert;
            },
            _ => {}
        }
    }
    Ok(())
}

async fn handle_query_input_insert_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        },
        KeyCode::Enter => {
            if let Err(e) = app.refresh_results().await {
                logging::error(&format!("Error refreshing results: {}", e));
            }
            app.input_mode = InputMode::Normal;
        },
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
        _ => Ok(()) // Noop for other modes
    }
}

async fn handle_results_input_normal_mode(key: KeyEvent, app: &mut App) -> Result<(), io::Error> {
    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
        match action {
            Action::Navigation(nav_action) => {
                match nav_action {
                    NavigationAction::Direction(direction) => {
                        app.move_cursor_in_results(direction);
                    },
                    NavigationAction::FocusPane(pane) => {
                        app.active_pane = pane;
                    }
                }
            },
            Action::Sort => {
                if let Err(e) = app.sort_results().await {
                    logging::error(&format!("Error sorting results: {}", e));
                }
            },
            Action::FirstPage => {
                if let Err(e) = app.first_page().await {
                    logging::error(&format!("Error going to first page: {}", e));
                }
            },
            Action::LastPage => {
                if let Err(e) = app.last_page().await {
                    logging::error(&format!("Error going to last page: {}", e));
                }
            },
            Action::NextPage => {
                if let Err(e) = app.next_page().await {
                    logging::error(&format!("Error going to next page: {}", e));
                }
            },
            Action::PreviousPage => {
                if let Err(e) = app.previous_page().await {
                    logging::error(&format!("Error going to previous page: {}", e));
                }
            },
            Action::NextTab => app.select_next_tab(),
            Action::PreviousTab => app.select_previous_tab(),
            _ => {}
        }
    }
    Ok(())
}
