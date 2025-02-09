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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

use crate::app::{App, InputMode, ActiveBlock};
use crate::ui::types::{Pane, Direction};
use crate::input::{Action, NavigationAction};


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
        terminal.draw(|f| ui::render(f, &app))?;


        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => {
                    // **First handle the Shift + key (pane switching)**
                    if let Some(action) = app.config.keymap.get_action(key.code, key.modifiers) {
                       match action {
                            Action::Navigation(nav_action) => {
                                match nav_action {
                                    NavigationAction::Direction(direction) => {
                                        match app.active_pane {
                                            Pane::Results => {
                                                app.move_cursor_in_results(direction);
                                            },
                                            Pane::Connections => {
                                                match direction {
                                                    Direction::Up => app.move_selection_up(),
                                                    Direction::Down => app.move_selection_down(),
                                                    _ => {} // Left/Right handled by TreeAction
                                                }
                                            },
                                            Pane::QueryInput => {
                                                app.handle_navigation(nav_action);
                                            },
                                            _ => {}
                                        }
                                    },
                                    NavigationAction::FocusPane(pane) => {
                                        app.active_pane = pane;
                                        // Reset cursor position when switching to Results pane
                                        if pane == Pane::Results {
                                            app.cursor_position = (0, 0);
                                        }
                                    }
                                }
                            }
                            Action::TreeAction(tree_action) => {
                                if let Err(e) = app.handle_tree_action(tree_action).await {
                                    logging::error(&format!("Error in tree action: {}", e));
                                }
                            },
                            Action::Sort => {
                                // When sort key is pressed in Normal mode, trigger sort logic
                                if let Err(e) = app.sort_results().await {
                                    logging::error(&format!("Error sorting results: {}", e));
                                }
                            },
                            Action::NextTab => {
                                app.select_next_tab();
                            },
                            Action::PreviousTab => {
                                app.select_previous_tab();
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
                        }
                    } else {
                        // Handle other normal mode keys
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.is_empty() => {
                                app.quit();
                            },
                            KeyCode::Char('a') if key.modifiers.is_empty() => {
                                app.show_connection_modal = true;
                                app.active_pane = Pane::ConnectionModal;
                            },
                            KeyCode::Char('i') if key.modifiers.is_empty() => {
                                app.input_mode = InputMode::Insert;
                            },
                            KeyCode::Char(':') if key.modifiers.is_empty() => {
                                app.input_mode = InputMode::Command;
                            },
                            _ => {}
                        }
                    }
                },
                    InputMode::Insert => {
                    match app.active_block {
                        ActiveBlock::ConnectionModal => {
                            match key.code {
                                KeyCode::Enter => {
                                    // Save the connection (we'll implement this later)
                                    app.save_connection(); // Call save_connection
                                    app.toggle_connection_modal();
                                }
                                KeyCode::Char(c) => {
                                    // Update the connection form based on the active field
                                    match app.connection_form.current_field {
                                        0 => app.connection_form.name.push(c),
                                        1 => app.connection_form.host.push(c),
                                        2 => app.connection_form.port.push(c),
                                        3 => app.connection_form.username.push(c),
                                        4 => app.connection_form.password.push(c),
                                        5 => app.connection_form.database.push(c),
                                        _ => {}
                                    }
                                }
                                KeyCode::Backspace => {
                                    // Handle backspace to delete characters
                                    match app.connection_form.current_field {
                                        0 => { app.connection_form.name.pop(); }
                                        1 => { app.connection_form.host.pop(); }
                                        2 => { app.connection_form.port.pop(); }
                                        3 => { app.connection_form.username.pop(); }
                                        4 => { app.connection_form.password.pop(); }
                                        5 => { app.connection_form.database.pop(); }
                                        _ => {}
                                    }
                                }
                                KeyCode::Esc => {
                                    // Handle escape to close the modal
                                    app.toggle_connection_modal();
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                        app.connection_form.current_field = (app.connection_form.current_field + 1) % 6; // Cycle through 6 fields
                                    }
                                    KeyCode::Up | KeyCode::Char('k') => {
                                       app.connection_form.current_field = (app.connection_form.current_field + 5) % 6; // Cycle backwards (add 5 to avoid negative modulo)
                                    }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    match app.active_pane {
                        Pane::QueryInput => {
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
                                KeyCode::Char(c) => {
                                    match c {
                                        c if c == app.config.keymap.up_key => {
                                            app.handle_navigation(NavigationAction::Direction(Direction::Up));
                                        },
                                        c if c == app.config.keymap.down_key => {
                                            app.handle_navigation(NavigationAction::Direction(Direction::Down));
                                        },
                                        c if c == app.config.keymap.left_key => {
                                            app.handle_navigation(NavigationAction::Direction(Direction::Left));
                                        },
                                        c if c == app.config.keymap.right_key => {
                                            app.handle_navigation(NavigationAction::Direction(Direction::Right));
                                        },
                                        _ => app.insert_char(c),
                                    }
                                },
                                KeyCode::Backspace => {
                                    app.delete_char();
                                },
                                KeyCode::Up => {
                                    app.handle_navigation(NavigationAction::Direction(Direction::Up));
                                },
                                KeyCode::Down => {
                                    app.handle_navigation(NavigationAction::Direction(Direction::Down));
                                },
                                KeyCode::Left => {
                                    app.handle_navigation(NavigationAction::Direction(Direction::Left));
                                },
                                KeyCode::Right => {
                                    app.handle_navigation(NavigationAction::Direction(Direction::Right));
                                },
                                _ => {}
                            }
                        },
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

