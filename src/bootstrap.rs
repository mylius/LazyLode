use std::io::{self, Stdout};
use std::panic;

use anyhow::{Context, Result};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::logging;

pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn install_panic_hook() {
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

        logging::error(&format!(
            "PANIC:\nMessage: {}\nLocation: {}",
            panic_message, location
        ));

        if std::env::var("RUST_BACKTRACE").unwrap_or_default() == "1" {
            logging::error(&format!(
                "Backtrace:\n{:?}",
                std::backtrace::Backtrace::capture()
            ));
        }
    }));
}

pub struct TerminalSession {
    terminal: AppTerminal,
}

impl TerminalSession {
    pub fn new() -> Result<Self> {
        enable_raw_mode().context("Failed to enable raw mode")?;
        logging::debug("Enabled raw mode");

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;
        logging::debug("Entered alternate screen");

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).context("Failed to create terminal")?;
        logging::debug("Created terminal");

        Ok(Self { terminal })
    }

    pub fn terminal_mut(&mut self) -> &mut AppTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if let Err(err) = disable_raw_mode() {
            let _ = logging::error(&format!("Failed to disable raw mode: {}", err));
        }

        let backend = self.terminal.backend_mut();
        if let Err(err) = execute!(backend, LeaveAlternateScreen, DisableMouseCapture) {
            let _ = logging::error(&format!("Failed to leave alternate screen: {}", err));
        }

        if let Err(err) = self.terminal.show_cursor() {
            let _ = logging::error(&format!("Failed to show cursor: {}", err));
        }
    }
}
