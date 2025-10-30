use ratatui::layout::{Constraint, Layout, Position, Rect};
use std::rc::Rc;

use crate::app::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryField {
    Where,
    OrderBy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginationControl {
    First,
    Previous,
    Next,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Connections(usize),
    QueryInput(QueryField, usize),
    Results(usize, usize),
    ResultTabs(usize),
    Pagination(PaginationControl),
    None,
}

pub struct LayoutContext {
    root: Rect,
    vertical_chunks: Rc<[Rect]>,
    main_chunks: Rc<[Rect]>,
    sidebar_chunks: Rc<[Rect]>,
    main_panel_chunks: Rc<[Rect]>,
}

impl LayoutContext {
    pub fn new(root: Rect) -> Self {
        let vertical_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(root);

        let main_area = vertical_chunks[1];
        let main_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
            .split(main_area);

        let sidebar_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(main_chunks[0]);

        let main_panel_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(1)])
            .split(main_chunks[1]);

        Self {
            root,
            vertical_chunks,
            main_chunks,
            sidebar_chunks,
            main_panel_chunks,
        }
    }

    pub fn with_app(root: Rect, _app: &App) -> Self {
        Self::new(root)
    }

    pub fn locate(&self, column: u16, row: u16, app: &App) -> Hit {
        let position = Position::new(column, row);

        if app.modal_manager.active_blocks_interaction() {
            return Hit::None;
        }

        if let Some(hit) = self
            .sidebar_inner()
            .filter(|rect| rect.contains(position))
            .and_then(|rect| self.hit_connections(rect, position, app))
        {
            return hit;
        }

        if self.query_area().contains(position) {
            return self.hit_query(position, app);
        }

        if self.results_area(app).contains(position) {
            return self.hit_results(position, app);
        }

        if self
            .tabs_area(app)
            .map_or(false, |area| area.contains(position))
        {
            return self.hit_tabs(position, app);
        }

        if self.pagination_area(app).contains(position) {
            return self.hit_pagination(position, app);
        }

        Hit::None
    }

    fn sidebar_inner(&self) -> Option<Rect> {
        let block = ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL);
        Some(block.inner(self.sidebar_chunks[1]))
    }

    fn hit_connections(&self, area: Rect, position: Position, app: &App) -> Option<Hit> {
        let relative_y = position.y.saturating_sub(area.y);
        let index = relative_y as usize;
        if index >= app.get_total_visible_items() {
            return None;
        }
        Some(Hit::Connections(index))
    }

    fn query_area(&self) -> Rect {
        self.main_panel_chunks[0]
    }

    fn hit_query(&self, position: Position, app: &App) -> Hit {
        let area = self.query_area();
        let relative_y = position.y.saturating_sub(area.y);
        let relative_x = position.x.saturating_sub(area.x) as usize;
        let where_height = area.height / 2;
        let field = if relative_y < where_height {
            QueryField::Where
        } else {
            QueryField::OrderBy
        };

        let max_len = match field {
            QueryField::Where => app
                .current_query_state()
                .map(|state| state.where_clause.len())
                .unwrap_or(0),
            QueryField::OrderBy => app
                .current_query_state()
                .map(|state| state.order_by_clause.len())
                .unwrap_or(0),
        };

        Hit::QueryInput(field, relative_x.min(max_len))
    }

    fn results_area(&self, app: &App) -> Rect {
        let chunks = self.result_panel_chunks(app);
        if app.result_tabs.is_empty() {
            chunks[0]
        } else {
            chunks[1]
        }
    }

    fn hit_results(&self, position: Position, app: &App) -> Hit {
        let area = self.results_area(app);
        let relative_x = position.x.saturating_sub(area.x);
        let table_inner = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .inner(area);

        let column = self.hit_result_column(relative_x, table_inner.width, app);
        let row = self.hit_result_row(position, table_inner, app);

        Hit::Results(column, row)
    }

    fn hit_result_column(&self, relative_x: u16, width: u16, app: &App) -> usize {
        let Some(tab_index) = app.selected_result_tab_index else {
            return 0;
        };
        let Some((_, result, _)) = app.result_tabs.get(tab_index) else {
            return 0;
        };

        let line_num_width = result.rows.len().to_string().len().max(3) as u16 + 1;
        if relative_x <= line_num_width {
            return 0;
        }

        let inner_width = width.saturating_sub(2);
        let data_cols = result.columns.len() as u16;
        if data_cols == 0 {
            return 0;
        }

        let spacing: u16 = 1;
        let total_spacing = spacing.saturating_mul(data_cols.saturating_sub(1));
        let remaining_width = inner_width
            .saturating_sub(line_num_width)
            .saturating_sub(total_spacing);
        let base = remaining_width / data_cols;
        let remainder = remaining_width % data_cols;

        let mut accum: u16 = line_num_width;
        for (index, column_width) in (0..data_cols)
            .map(|i| base + if i < remainder { 1 } else { 0 })
            .enumerate()
        {
            accum = accum.saturating_add(column_width);
            if relative_x < accum {
                return index;
            }
            accum = accum.saturating_add(spacing);
        }

        (data_cols - 1) as usize
    }

    fn hit_result_row(&self, position: Position, table_inner: Rect, app: &App) -> usize {
        const HEADER_HEIGHT: u16 = 1;

        let Some(tab_index) = app.selected_result_tab_index else {
            return 0;
        };
        let Some((_, result, _)) = app.result_tabs.get(tab_index) else {
            return 0;
        };

        let total_rows = result.rows.len();
        if total_rows == 0 {
            return 0;
        }

        let data_y = position.y.saturating_sub(table_inner.y);
        let row_in_view = usize::from(data_y.saturating_sub(HEADER_HEIGHT));
        let visible_capacity = usize::from(table_inner.height.saturating_sub(HEADER_HEIGHT));

        let cursor_row = app.cursor_position.1.min(total_rows.saturating_sub(1));
        let start_row = if visible_capacity == 0 {
            0
        } else {
            cursor_row
                .saturating_sub(visible_capacity / 2)
                .min(total_rows.saturating_sub(visible_capacity))
        };

        start_row
            .saturating_add(row_in_view)
            .min(total_rows.saturating_sub(1))
    }

    fn tabs_area(&self, app: &App) -> Option<Rect> {
        if app.result_tabs.is_empty() {
            None
        } else {
            Some(self.result_panel_chunks(app)[0])
        }
    }

    fn hit_tabs(&self, position: Position, app: &App) -> Hit {
        let Some(area) = self.tabs_area(app) else {
            return Hit::None;
        };
        let tab_count = app.result_tabs.len();
        if tab_count == 0 {
            return Hit::None;
        }

        let available_width = area.width.saturating_sub(4);
        let divider_width = 3;
        let total_divider_width = if tab_count > 1 {
            (tab_count as u16 - 1).saturating_mul(divider_width)
        } else {
            0
        };
        let max_tab_width = if tab_count > 0 {
            (available_width.saturating_sub(total_divider_width) / tab_count as u16).max(8)
        } else {
            8
        };

        let mut widths = Vec::with_capacity(tab_count);
        for (name, _, _) in app.result_tabs.iter() {
            let shortened = crate::ui::shorten_tab_name_intelligent(
                name,
                &app.result_tabs,
                max_tab_width as usize,
            );
            widths.push(shortened.len() as u16);
        }

        let mut accum = 0;
        let mut index = 0;
        let x = position.x.saturating_sub(area.x);

        while index < tab_count {
            accum += widths[index];
            if x < accum {
                return Hit::ResultTabs(index);
            }
            accum += divider_width;
            if x < accum {
                return Hit::ResultTabs(index);
            }
            index += 1;
        }

        Hit::ResultTabs(tab_count.saturating_sub(1))
    }

    fn pagination_area(&self, app: &App) -> Rect {
        let chunks = self.result_panel_chunks(app);
        if app.result_tabs.is_empty() {
            chunks[1]
        } else {
            chunks[2]
        }
    }

    fn hit_pagination(&self, position: Position, app: &App) -> Hit {
        let area = self.pagination_area(app);
        let width = area.width;
        if width == 0 {
            return Hit::None;
        }

        let button_width = (width / 4).max(1);
        let index = position.x.saturating_sub(area.x) / button_width;

        let control = match index {
            0 => PaginationControl::First,
            1 => PaginationControl::Previous,
            2 => PaginationControl::Next,
            _ => PaginationControl::Last,
        };

        Hit::Pagination(control)
    }

    fn result_panel_chunks(&self, app: &App) -> Rc<[Rect]> {
        let results_panel = self.main_panel_chunks[1];
        if app.result_tabs.is_empty() {
            Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(results_panel)
        } else {
            Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(results_panel)
        }
    }
}
