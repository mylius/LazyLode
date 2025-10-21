use std::io;

use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crossterm::execute;
use futures::executor;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::app::{App, InputMode};
use crate::input;
use crate::logging;
use crate::navigation::NavigationInputHandler;
use crate::ui;
use crate::ui::types::Pane;

pub struct Runner<'a, B: Backend> {
    terminal: &'a mut Terminal<B>,
    app: App,
}

impl<'a, B: Backend> Runner<'a, B> {
    pub fn new(terminal: &'a mut Terminal<B>, app: App) -> Self {
        Self { terminal, app }
    }

    pub async fn run(mut self) -> Result<(), io::Error> {
        loop {
            self.tick().await?;

            if self.app.should_quit {
                return Ok(());
            }
        }
    }

    async fn tick(&mut self) -> Result<(), io::Error> {
        if let Err(err) = self.app.check_background_prefetching() {
            let _ = logging::error(&format!("Error checking background prefetching: {}", err));
        }

        self.terminal
            .draw(|frame| ui::render(frame, &mut self.app))?;

        self.refresh_cursor_style();

        match event::read()? {
            Event::Key(key) => self.handle_key(key).await,
            Event::Mouse(event) => match event.kind {
                MouseEventKind::ScrollUp => self.handle_scroll_up().await,
                MouseEventKind::ScrollDown => self.handle_scroll_down().await,
                MouseEventKind::Down(MouseButton::Left) => self.handle_mouse_click(event).await,
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }

    fn refresh_cursor_style(&self) {
        let cursor_style = match self.app.input_mode {
            InputMode::Normal => SetCursorStyle::SteadyBlock,
            _ => SetCursorStyle::SteadyBar,
        };
        let _ = execute!(io::stdout(), cursor_style);
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<(), io::Error> {
        NavigationInputHandler::handle_key(key.code, key.modifiers, &mut self.app)
            .await
            .map_err(|err| {
                let _ = logging::error(&format!("Error handling key input: {}", err));
                io::Error::new(io::ErrorKind::Other, err)
            })
    }

    async fn handle_mouse(&mut self, event: MouseEvent) -> Result<(), io::Error> {
        let size = self.terminal.size()?;
        let layout = ui::layout::LayoutContext::new(Rect::new(0, 0, size.width, size.height));
        match layout.locate(event.column, event.row, &self.app) {
            ui::layout::Hit::Connections(index) => {
                self.app.select_connection(index);
                self.app.focus_connections();
                self.expand_selected_connection().await
            }
            ui::layout::Hit::QueryInput(field, position) => {
                self.app.focus_query_input(field, position);
                Ok(())
            }
            ui::layout::Hit::Results(column, row) => {
                self.app.focus_results(column, row);
                Ok(())
            }
            ui::layout::Hit::ResultTabs(tab) => {
                self.app.select_tab(tab);
                self.app.focus_results(0, 0);
                Ok(())
            }
            ui::layout::Hit::Pagination(control) => self.handle_pagination(control).await,
            ui::layout::Hit::None => Ok(()),
        }
    }

    async fn handle_mouse_click(&mut self, event: MouseEvent) -> Result<(), io::Error> {
        let size = self.terminal.size()?;
        let layout = ui::layout::LayoutContext::new(Rect::new(0, 0, size.width, size.height));
        match layout.locate(event.column, event.row, &self.app) {
            ui::layout::Hit::Connections(index) => {
                self.app.select_connection(index);
                self.app.focus_connections();
                self.expand_selected_connection().await
            }
            ui::layout::Hit::QueryInput(field, position) => {
                self.app.focus_query_input(field, position);
                Ok(())
            }
            ui::layout::Hit::Results(column, row) => {
                self.app.focus_results(column, row);
                Ok(())
            }
            ui::layout::Hit::ResultTabs(tab) => {
                self.app.select_tab(tab);
                self.app.focus_results(0, 0);
                Ok(())
            }
            ui::layout::Hit::Pagination(control) => self.handle_pagination(control).await,
            ui::layout::Hit::None => Ok(()),
        }
    }

    async fn expand_selected_connection(&mut self) -> Result<(), io::Error> {
        executor::block_on(self.app.handle_tree_action(input::TreeAction::Expand)).map_err(|err| {
            let _ = logging::error(&format!("Error expanding tree item: {}", err));
            io::Error::new(io::ErrorKind::Other, err)
        })
    }

    async fn handle_pagination(
        &mut self,
        control: ui::layout::PaginationControl,
    ) -> Result<(), io::Error> {
        use ui::layout::PaginationControl::*;

        match control {
            First => self.app.first_page().await.map(|_| ()),
            Previous => self.app.previous_page().await.map(|_| ()),
            Next => self.app.next_page().await.map(|_| ()),
            Last => self.app.last_page().await.map(|_| ()),
        }
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }

    async fn handle_scroll_up(&mut self) -> Result<(), io::Error> {
        match self.app.active_pane {
            Pane::Connections => {
                self.app.select_previous_connection();
                Ok(())
            }
            Pane::Results => self
                .app
                .previous_page()
                .await
                .map(|_| ())
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err)),
            _ => Ok(()),
        }
    }

    async fn handle_scroll_down(&mut self) -> Result<(), io::Error> {
        match self.app.active_pane {
            Pane::Connections => {
                self.app.select_next_connection();
                Ok(())
            }
            Pane::Results => self
                .app
                .next_page()
                .await
                .map(|_| ())
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err)),
            _ => Ok(()),
        }
    }
}
