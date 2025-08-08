use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
    Frame,
};

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_connection_modal(frame: &mut Frame, app: &crate::app::App) {
    let area = centered_rect(60, 50, frame.area());

    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .title("New Connection")
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(app.config.theme.text_color())
                    .bg(app.config.theme.surface1_color()),
            ),
        area,
    );

    let modal_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    let fields = [
        ("Name:", &app.connection_form.name),
        ("Host:", &app.connection_form.host),
        ("Port:", &app.connection_form.port),
        ("Username:", &app.connection_form.username),
        ("Password:", &app.connection_form.password),
        ("Database:", &app.connection_form.database),
    ];

    for (i, (label, value)) in fields.iter().enumerate() {
        let content = if *label == "Password:" {
            "*".repeat(value.len())
        } else {
            value.to_string()
        };

        let style = if i == app.connection_form.current_field {
            Style::default().fg(app.config.theme.accent_color())
        } else {
            Style::default().fg(app.config.theme.text_color())
        };

        frame.render_widget(
            Paragraph::new(format!("{} {}", label, content)).style(style),
            modal_layout[i],
        );
    }
}

pub fn render_deletion_modal(frame: &mut Frame, app: &crate::app::App) {
    let area = centered_rect(70, 60, frame.area());

    frame.render_widget(Clear, area);

    // Create modal block
    let block = Block::default()
        .title("Confirm Deletion")
        .borders(Borders::ALL)
        .style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface1_color()),
        );

    frame.render_widget(block.clone(), area);

    // Get inner area for content
    let inner_area = block.inner(area);

    // Create layout for header, table, and confirmation message
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Header
            Constraint::Min(3),    // Table
            Constraint::Length(2), // Footer
        ])
        .split(inner_area);

    // Render header
    let header = "The following rows will be deleted:";
    frame.render_widget(
        Paragraph::new(header).style(Style::default().fg(app.config.theme.text_color())),
        chunks[0],
    );

    // Get and render preview data
    if let Some(preview_data) = app.get_deletion_preview() {
        if let Some((_, result, _)) = app
            .selected_result_tab_index
            .and_then(|idx| app.result_tabs.get(idx))
        {
            let header = Row::new(
                result
                    .columns
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
            )
            .style(
                Style::default()
                    .fg(app.config.theme.accent_color())
                    .add_modifier(Modifier::BOLD),
            );

            let rows: Vec<Row> = preview_data
                .iter()
                .map(|row| {
                    Row::new(row.iter().map(|cell| cell.as_str()).collect::<Vec<_>>())
                        .style(Style::default().fg(app.config.theme.text_color()))
                })
                .collect();

            let widths = vec![
                Constraint::Percentage(100 / result.columns.len() as u16);
                result.columns.len()
            ];

            let table = Table::new(rows, widths)
                .header(header)
                .style(Style::default().bg(app.config.theme.surface1_color()));

            frame.render_widget(table, chunks[1]);
        }
    }

    // Render footer with confirmation message
    let footer = "Press Enter to confirm deletion, Esc to cancel";
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(app.config.theme.text_color())),
        chunks[2],
    );
}
