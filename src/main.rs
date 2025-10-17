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
