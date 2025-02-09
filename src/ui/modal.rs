use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style},
    widgets::{Block, Borders, Clear, Paragraph},
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
    let area = centered_rect(60, 50, frame.size());
    
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .title("New Connection")
            .borders(Borders::ALL)
            .style(Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface1_color())),
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
            Style::default()
                .fg(app.config.theme.accent_color())
        } else {
            Style::default()
                .fg(app.config.theme.text_color())
        };

        frame.render_widget(
            Paragraph::new(format!("{} {}", label, content))
                .style(style),
            modal_layout[i],
        );
    }
}
