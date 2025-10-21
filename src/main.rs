mod app;
mod bootstrap;
mod command;
mod config;
mod database;
mod input;
mod logging;
mod navigation;
mod runtime;
mod theme;
mod ui;

use crate::app::App;
use crate::bootstrap::{install_panic_hook, TerminalSession};
use crate::runtime::Runner;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    logging::init_logger()?;
    install_panic_hook();
    logging::info("Starting LazyLode Database Explorer")?;

    let mut session = TerminalSession::new()?;
    let app = App::new_with_async_connections().await?;

    let res = Runner::new(session.terminal_mut(), app).run().await;

    if let Err(err) = res {
        logging::error(&format!("Application error: {}", err))?;
        return Err(anyhow::anyhow!(err));
    }

    logging::info("Application terminated successfully")?;
    Ok(())
}
