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
            Constraint::Length(3),
        ])
        .split(area);

    let ssh_tunnel_label = "SSH Tunnel:".to_string();
    let ssh_tunnel_value = app
        .connection_form
        .ssh_tunnel_name
        .clone()
        .unwrap_or_else(|| "None".to_string());

    let fields: Vec<(String, String)> = vec![
        ("Name:".into(), app.connection_form.name.clone()),
        ("Host:".into(), app.connection_form.host.clone()),
        ("Port:".into(), app.connection_form.port.clone()),
        ("Username:".into(), app.connection_form.username.clone()),
        (
            "Password:".into(),
            "*".repeat(app.connection_form.password.len()),
        ),
        ("Database:".into(), app.connection_form.database.clone()),
        (ssh_tunnel_label, ssh_tunnel_value),
    ];

    for (i, (label, value)) in fields.iter().enumerate() {
        let style = if i == app.connection_form.current_field {
            Style::default().fg(app.config.theme.accent_color())
        } else {
            Style::default().fg(app.config.theme.text_color())
        };

        frame.render_widget(
            Paragraph::new(format!("{} {}", label, value)).style(style),
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

pub fn render_themes_modal(frame: &mut Frame, app: &crate::app::App) {
    let area = centered_rect(50, 60, frame.area());

    frame.render_widget(Clear, area);

    // Create modal block
    let block = Block::default()
        .title("Available Themes")
        .borders(Borders::ALL)
        .style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface1_color()),
        );

    frame.render_widget(block.clone(), area);

    // Get inner area for content
    let inner_area = block.inner(area);

    // Create layout for header and theme list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Header
            Constraint::Min(3),    // Theme list
            Constraint::Length(2), // Footer
        ])
        .split(inner_area);

    // Render header
    let header = "Select a theme to switch to:";
    frame.render_widget(
        Paragraph::new(header).style(Style::default().fg(app.config.theme.text_color())),
        chunks[0],
    );

    // Get and render themes
    if let Ok(themes) = crate::config::Config::list_themes() {
        if themes.is_empty() {
            let no_themes = "No themes available";
            frame.render_widget(
                Paragraph::new(no_themes).style(Style::default().fg(app.config.theme.text_color())),
                chunks[1],
            );
        } else {
            // Create a list of theme items
            let theme_items: Vec<_> = themes
                .iter()
                .map(|theme| {
                    let is_current = theme == &app.config.theme_name;
                    let display_text = if is_current {
                        format!("{} (current)", theme)
                    } else {
                        theme.clone()
                    };
                    
                    Paragraph::new(display_text).style(
                        Style::default()
                            .fg(if is_current {
                                app.config.theme.accent_color()
                            } else {
                                app.config.theme.text_color()
                            })
                            .add_modifier(if is_current { Modifier::BOLD } else { Modifier::empty() }),
                    )
                })
                .collect();

            // Render themes in a scrollable area
            let theme_list_height = themes.len().min(15); // Limit to 15 themes for display
            let theme_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    (0..theme_list_height)
                        .map(|_| Constraint::Length(1))
                        .collect::<Vec<_>>(),
                )
                .split(chunks[1]);

            for (i, theme_item) in theme_items.iter().enumerate() {
                if i < theme_chunks.len() {
                    frame.render_widget(theme_item.clone(), theme_chunks[i]);
                }
            }
        }
    }

    // Render footer with instructions
    let footer = "Press Esc to close";
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(app.config.theme.text_color())),
        chunks[2],
    );
}
