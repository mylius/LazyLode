use crate::app::App;
use crate::ui::types::Pane;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

pub struct ResultsPane;

impl ResultsPane {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, app: &App, area: Rect) {
        let has_tabs = !app.result_tabs.is_empty();
        let constraints = if has_tabs {
            vec![
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ]
        } else {
            vec![Constraint::Min(1), Constraint::Length(3)]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut index = 0;
        if has_tabs {
            self.render_result_tabs(frame, app, chunks[index]);
            index += 1;
        }

        self.render_results(frame, app, chunks[index]);
        index += 1;
        self.render_pagination(frame, app, chunks[index]);
    }

    fn render_result_tabs(&self, frame: &mut Frame, app: &App, area: Rect) {
        if app.result_tabs.is_empty() {
            return;
        }

        let available_width = area.width.saturating_sub(4);
        let tab_count = app.result_tabs.len();
        let divider_width = 3;
        let total_divider_width = if tab_count > 1 {
            (tab_count - 1) * divider_width
        } else {
            0
        };
        let max_tab_width = if tab_count > 0 {
            (available_width.saturating_sub(total_divider_width as u16) / tab_count as u16).max(8)
        } else {
            8
        };

        let tab_titles: Vec<_> = app
            .result_tabs
            .iter()
            .enumerate()
            .map(|(index, (name, _, _))| {
                let shortened_name = crate::ui::shorten_tab_name_intelligent(
                    name,
                    &app.result_tabs,
                    max_tab_width as usize,
                );
                let color = crate::ui::get_tab_color(name, index);
                Line::from(Span::styled(shortened_name, Style::default().fg(color)))
            })
            .collect();

        let tabs = Tabs::new(tab_titles)
            .select(app.selected_result_tab_index.unwrap_or(0))
            .block(Block::default().borders(Borders::BOTTOM))
            .style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface0_color()),
            )
            .highlight_style(
                Style::default()
                    .fg(app.config.theme.accent_color())
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::raw(" | "));

        frame.render_widget(tabs, area);
    }

    fn render_results(&self, frame: &mut Frame, app: &App, area: Rect) {
        let results_nav_info = if app.active_pane == Pane::Results {
            format!(" [{}]", app.navigation_manager.get_navigation_info())
        } else {
            String::new()
        };

        let results_title = format!("Results{}", results_nav_info);
        let mut block = Block::default()
            .title(results_title)
            .borders(Borders::ALL)
            .title_style(
                Style::default()
                    .fg(app.config.theme.header_fg_color())
                    .bg(app.config.theme.header_bg_color()),
            );
        if app.active_pane == Pane::Results {
            block = block.border_style(Style::default().fg(app.config.theme.accent_color()));
        }
        let current_result = app.selected_result_tab_index.and_then(|tab_index| {
            app.result_tabs
                .get(tab_index)
                .map(|(_, result, state)| (result, state))
        });
        if let Some((result, query_state)) = current_result {
            let header = result
                .columns
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>();
            if header.is_empty() {
                frame.render_widget(
                    Paragraph::new("No results to display.")
                        .style(Style::default().fg(app.config.theme.text_color())),
                    area,
                );
                return;
            }

            let max_lines = result.rows.len();
            let line_num_width = max_lines.to_string().len().max(3) as u16;

            let spacing: u16 = 1;
            let table_inner = block.inner(area);
            let first_col_w = line_num_width + 1;
            let mut widths: Vec<Constraint> = Vec::with_capacity(1 + header.len());
            widths.push(Constraint::Length(first_col_w));
            let data_cols = header.len() as u16;
            if data_cols > 0 {
                let total_spacing = spacing.saturating_mul(data_cols.saturating_sub(1));
                let remaining_w = table_inner
                    .width
                    .saturating_sub(first_col_w)
                    .saturating_sub(total_spacing);
                let base = if data_cols > 0 {
                    remaining_w / data_cols
                } else {
                    0
                };
                let rem = if data_cols > 0 {
                    remaining_w % data_cols
                } else {
                    0
                };
                for i in 0..data_cols {
                    let w = base + if i < rem { 1 } else { 0 };
                    widths.push(Constraint::Length(w));
                }
            }

            let mut header_cells = vec![Cell::from("#").style(
                Style::default()
                    .fg(app.config.theme.accent_color())
                    .add_modifier(Modifier::BOLD),
            )];

            header_cells.extend(header.iter().map(|&h| {
                Cell::from(h).style(
                    Style::default()
                        .fg(app.config.theme.accent_color())
                        .add_modifier(Modifier::BOLD),
                )
            }));

            let header_row = Row::new(header_cells);

            let header_rows: u16 = 1;
            let visible_capacity = table_inner.height.saturating_sub(header_rows).max(0) as usize;

            let total_rows = result.rows.len();
            let max_start = total_rows.saturating_sub(visible_capacity);
            let cursor_row = app.cursor_position.1.min(total_rows.saturating_sub(1));
            let start_row = if visible_capacity == 0 {
                0
            } else {
                cursor_row
                    .saturating_sub(visible_capacity / 2)
                    .min(max_start)
            };

            let rows: Vec<Row> = result
                .rows
                .iter()
                .enumerate()
                .skip(start_row)
                .take(visible_capacity)
                .map(|(row_idx, row)| {
                    let is_marked = query_state.rows_marked_for_deletion.contains(&row_idx);
                    let is_selected =
                        app.active_pane == Pane::Results && row_idx == app.cursor_position.1;

                    let base_bg = if is_marked {
                        Color::Rgb(139, 0, 0)
                    } else if is_selected {
                        app.config.theme.accent_color()
                    } else if (row_idx + start_row) % 2 == 0 {
                        app.config.theme.row_even_bg_color()
                    } else {
                        app.config.theme.row_odd_bg_color()
                    };

                    let mut row_cells = vec![Cell::from(format!(
                        "{:>width$}",
                        row_idx + 1,
                        width = line_num_width as usize
                    ))
                    .style(
                        Style::default()
                            .fg(app.config.theme.text_color())
                            .bg(base_bg),
                    )];

                    row_cells.extend(row.iter().enumerate().map(|(col_idx, cell)| {
                        let is_selected = app.active_pane == Pane::Results
                            && row_idx == app.cursor_position.1
                            && col_idx == app.cursor_position.0;
                        let is_marked = query_state.rows_marked_for_deletion.contains(&row_idx);

                        let base_bg = if is_marked {
                            Color::Rgb(139, 0, 0)
                        } else if is_selected {
                            app.config.theme.accent_color()
                        } else if (row_idx + start_row) % 2 == 0 {
                            app.config.theme.row_even_bg_color()
                        } else {
                            app.config.theme.row_odd_bg_color()
                        };

                        let style = Style::default()
                            .fg(app.config.theme.text_color())
                            .bg(base_bg);

                        Cell::from(cell.as_str()).style(style)
                    }));

                    Row::new(row_cells)
                })
                .collect();

            let table = Table::new(rows, widths)
                .header(header_row)
                .block(block)
                .column_spacing(spacing)
                .style(Style::default().bg(app.config.theme.surface0_color()));

            frame.render_widget(table, area);
        } else {
            frame.render_widget(
                Block::default()
                    .title("Results")
                    .borders(Borders::ALL)
                    .title_style(
                        Style::default()
                            .fg(app.config.theme.header_fg_color())
                            .bg(app.config.theme.header_bg_color()),
                    )
                    .style(
                        Style::default()
                            .fg(app.config.theme.text_color())
                            .bg(app.config.theme.surface0_color()),
                    ),
                area,
            );
        }
    }

    fn render_pagination(&self, frame: &mut Frame, app: &App, area: Rect) {
        let mut block = Block::default()
            .title("Pagination")
            .borders(Borders::ALL)
            .title_style(
                Style::default()
                    .fg(app.config.theme.header_fg_color())
                    .bg(app.config.theme.header_bg_color()),
            )
            .style(Style::default().bg(app.config.theme.surface0_color()));
        if app.active_pane == Pane::Results {
            block = block.border_style(Style::default().fg(app.config.theme.accent_color()));
        }

        let pagination_info = if let Some(state) = app.current_query_state() {
            format!(
                "Page: {}/{} | Size: {} | Total: {} | {}:First {}:Last {}:Prev {}:Next ",
                state.current_page,
                state.total_pages.unwrap_or(1),
                state.page_size,
                state.total_records.unwrap_or(0),
                app.config.keymap.first_page_key,
                app.config.keymap.last_page_key,
                app.config.keymap.prev_page_key,
                app.config.keymap.next_page_key,
            )
        } else {
            String::from("No active table")
        };

        frame.render_widget(
            Paragraph::new(pagination_info)
                .block(block)
                .style(Style::default().fg(app.config.theme.text_color())),
            area,
        );
    }
}
