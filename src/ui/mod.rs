use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph},
    Frame,
};

use ratatui::layout::Direction as LayoutDirection;

use crate::app::{App, InputMode};

pub mod components;
pub mod layout;
pub mod modal_manager;
pub mod modals;
pub mod panes;
pub mod types;

pub use types::Pane;

/// Renders the entire UI of the application.
pub fn render(frame: &mut Frame, app: &App) {
    // Set background color for the entire frame
    frame.render_widget(
        Block::default().style(Style::default().bg(app.config.theme.base_color())),
        frame.area(),
    );

    // Define vertical chunks for status bar and main content (no command bar)
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar height
            Constraint::Min(1),    // Main content area (flexible height)
        ])
        .split(frame.area());

    render_status_bar(frame, app, chunks[0]);
    render_main_content(frame, app, chunks[1]);

    app.modal_manager.render_all(frame, app);
}

/// Renders the status bar at the top of the UI.
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    // Determine mode indicator - check modal first, then navigation system
    let mode = if let Some(vim_mode) = app.modal_manager.get_active_mode() {
        match vim_mode {
            crate::navigation::types::VimMode::Normal => "NORMAL".to_string(),
            crate::navigation::types::VimMode::Insert => "INSERT".to_string(),
            crate::navigation::types::VimMode::Visual => "VISUAL".to_string(),
            crate::navigation::types::VimMode::Command => "COMMAND".to_string(),
        }
    } else {
        app.navigation_manager.get_mode_indicator()
    };

    // Determine what to show in the navigation info section
    let nav_info = if app.input_mode == InputMode::Command {
        "Command".to_string()
    } else if app.modal_manager.has_modals() {
        app.modal_manager
            .get_active_title()
            .unwrap_or_else(|| "Modal".to_string())
    } else {
        app.navigation_manager.get_navigation_info()
    };

    // Create status text, including current mode, navigation info, and status message
    let status = Line::from(format!(
        "{} | {} | {}",
        mode,
        nav_info,
        app.status_message.as_deref().unwrap_or("")
    ));

    frame.render_widget(
        Paragraph::new(status).style(
            Style::default()
                .fg(app.config.theme.text_color())
                .bg(app.config.theme.surface0_color()),
        ), // Style with theme colors
        area,
    );
}

/// Renders the main content area, split into sidebar and main panel.
fn render_main_content(frame: &mut Frame, app: &App, area: Rect) {
    // Split main area horizontally into sidebar (connections) and main panel (query, results)
    let horizontal_chunks = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Sidebar takes 20% width
            Constraint::Percentage(80), // Main panel takes 80% width
        ])
        .split(area);

    app.sidebar_pane.render(frame, app, horizontal_chunks[0]); // Render the sidebar (connections tree)
    render_main_panel(frame, app, horizontal_chunks[1]); // Render the main panel (query input, results)
}

fn render_main_panel(frame: &mut Frame, app: &App, area: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().bg(app.config.theme.base_color())),
        area,
    );

    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(1)])
        .split(area);

    app.results_pane.render(frame, app, chunks[1]);

    app.query_input_pane.render(frame, app, chunks[0]);
}

/// Intelligently shortens tab names to fit within the available width.
/// Analyzes all open tabs to determine what distinguishing information to preserve.
pub fn shorten_tab_name_intelligent(
    full_name: &str,
    all_tabs: &[(String, crate::database::QueryResult, crate::app::QueryState)],
    max_width: usize,
) -> String {
    if full_name.len() <= max_width {
        return full_name.to_string();
    }

    // Parse the full name: "connection:database:schema.table" or "connection:schema.table"
    let parts: Vec<&str> = full_name.split(':').collect();

    // Local helpers removed to avoid lifetime issues; inline splits below

    let others = all_tabs
        .iter()
        .map(|(n, _, _)| n.as_str())
        .filter(|&n| n != full_name)
        .collect::<Vec<_>>();

    if parts.len() == 2 {
        let connection = parts[0];
        let schema_table = parts[1];
        let (schema, table) = if let Some(dot_pos) = schema_table.rfind('.') {
            (&schema_table[..dot_pos], &schema_table[dot_pos + 1..])
        } else {
            ("", schema_table)
        };

        let needs_connection = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            ps.len() == 2 && ps[1] == schema_table
        });

        let needs_schema = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            match ps.len() {
                2 => ps[1]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[1][..p];
                        let otable = &ps[1][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                3 => ps[2]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[2][..p];
                        let otable = &ps[2][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                _ => false,
            }
        });

        let mut candidates: Vec<String> = Vec::new();
        if !needs_connection && !needs_schema {
            if table.len() <= max_width {
                return table.to_string();
            }
        }
        if needs_schema && schema_table.len() <= max_width {
            return schema_table.to_string();
        }

        let conn_abbrev = abbreviate_name(connection, 3);
        candidates.push(format!("{}:{}", conn_abbrev, schema_table));
        candidates.push(format!("{}:{}", conn_abbrev, table));

        if let Some(best) = candidates.into_iter().find(|c| c.len() <= max_width) {
            return best;
        }

        if max_width > 3 {
            return format!("{}...", &schema_table[..max_width.saturating_sub(3)]);
        }
    } else if parts.len() == 3 {
        let connection = parts[0];
        let database = parts[1];
        let schema_table = parts[2];
        let (schema, table) = if let Some(dot_pos) = schema_table.rfind('.') {
            (&schema_table[..dot_pos], &schema_table[dot_pos + 1..])
        } else {
            ("", schema_table)
        };

        let needs_connection = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            ps.len() == 3 && ps[1] == database && ps[2] == schema_table
        });
        let needs_database = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            (ps.len() == 2 && ps[1] == schema_table)
                || (ps.len() == 3 && ps[2] == schema_table && ps[1] != database)
        });
        let needs_schema = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            match ps.len() {
                3 => ps[2]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[2][..p];
                        let otable = &ps[2][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                2 => ps[1]
                    .rfind('.')
                    .map(|p| {
                        let oschema = &ps[1][..p];
                        let otable = &ps[1][p + 1..];
                        otable == table && oschema != schema
                    })
                    .unwrap_or(false),
                _ => false,
            }
        });
        let has_different_schemas = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            if ps.len() == 3 && ps[0] == connection && ps[1] == database {
                ps[2]
                    .rfind('.')
                    .map(|p| &ps[2][..p] != schema)
                    .unwrap_or(false)
            } else {
                false
            }
        });
        let has_multiple_databases_same_schema = others.iter().any(|&n| {
            let ps: Vec<&str> = n.split(':').collect();
            if ps.len() == 3 && ps[0] == connection && ps[1] != database {
                ps[2]
                    .rfind('.')
                    .map(|p| &ps[2][..p] == schema)
                    .unwrap_or(false)
            } else {
                false
            }
        });

        let mut candidates: Vec<String> = Vec::new();

        if has_multiple_databases_same_schema {
            candidates.push(format!("{}.{}", database, table));
            let db3 = abbreviate_name(database, 3);
            candidates.push(format!("{}.{}", db3, table));
            let sep = 1usize;
            if max_width > db3.len() + sep {
                let remain = max_width - db3.len() - sep;
                if remain > 1 {
                    let tfit = if table.len() > remain {
                        abbreviate_name(table, remain)
                    } else {
                        table.to_string()
                    };
                    candidates.push(format!("{}.{}", db3, tfit));
                }
            }
        }
        if has_different_schemas {
            candidates.push(schema_table.to_string());
            candidates.push(format!("{}.{}", abbreviate_name(schema, 3), table));
        }
        if !needs_connection && !needs_database && !needs_schema && !has_different_schemas {
            candidates.push(table.to_string());
        }
        if !needs_connection && needs_database {
            candidates.push(format!("{}.{}", database, table));
            candidates.push(format!("{}:{}", database, schema_table));
            candidates.push(format!("{}.{}", abbreviate_name(database, 3), table));
        }
        if needs_connection && !needs_database {
            let c3 = abbreviate_name(connection, 3);
            candidates.push(format!("{}:{}", c3, table));
            candidates.push(format!("{}:{}", c3, schema_table));
        }
        if needs_connection && needs_database {
            let c2 = abbreviate_name(connection, 2);
            let d3 = abbreviate_name(database, 3);
            candidates.push(format!("{}:{}:{}", c2, d3, schema_table));
            candidates.push(format!("{}:{}.{}", c2, d3, table));
        }

        // Generic abbreviated options
        let c2 = abbreviate_name(connection, 2);
        let d3 = abbreviate_name(database, 3);
        candidates.push(format!("{}:{}:{}", c2, d3, schema_table));
        candidates.push(format!("{}:{}.{}", c2, d3, table));
        candidates.push(format!("{}.{}", d3, table));

        if let Some(best) = candidates.into_iter().find(|c| c.len() <= max_width) {
            return best;
        }

        if has_multiple_databases_same_schema {
            let full = format!("{}.{}", database, table);
            if full.len() <= max_width {
                return full;
            }
            let d3 = abbreviate_name(database, 3);
            let d3_full = format!("{}.{}", d3, table);
            if d3_full.len() <= max_width {
                return d3_full;
            }
            if max_width > 2 {
                for dbl in (2..=3).rev() {
                    let dab = abbreviate_name(database, dbl);
                    if max_width > dab.len() + 1 {
                        let rem = max_width - dab.len() - 1;
                        let tab = if table.len() > rem {
                            abbreviate_name(table, rem)
                        } else {
                            table.to_string()
                        };
                        let cand = format!("{}.{}", dab, tab);
                        if cand.len() <= max_width {
                            return cand;
                        }
                    }
                }
                let db_only = abbreviate_name(database, max_width.min(3));
                if !db_only.is_empty() {
                    return db_only;
                }
            }
        }

        // As a last pass, try abbreviated conn/db variants
        let c2 = abbreviate_name(connection, 2);
        let d3 = abbreviate_name(database, 3);
        let full = format!("{}:{}:{}", c2, d3, schema_table);
        if full.len() <= max_width {
            return full;
        }
        if let Some(p) = schema_table.rfind('.') {
            let t = &schema_table[p + 1..];
            let conn_db_t = format!("{}:{}.{}", c2, d3, t);
            if conn_db_t.len() <= max_width {
                return conn_db_t;
            }
            let db_t = format!("{}.{}", d3, t);
            if db_t.len() <= max_width {
                return db_t;
            }
            let c_t = format!("{}:{}", c2, t);
            if c_t.len() <= max_width {
                return c_t;
            }
        }

        if max_width > 3 {
            return format!("{}...", &schema_table[..max_width.saturating_sub(3)]);
        }
    }

    if max_width > 3 {
        format!("{}...", &full_name[..max_width.saturating_sub(3)])
    } else {
        "..".to_string()
    }
}

/// Abbreviates a name to the specified length, taking characters from the beginning and end
pub fn abbreviate_name(name: &str, target_length: usize) -> String {
    if name.len() <= target_length {
        return name.to_string();
    }

    if target_length <= 2 {
        return name[..target_length].to_string();
    }

    // Take first and last characters with ellipsis in between
    let first_chars = (target_length + 1) / 2;
    let last_chars = target_length - first_chars - 1;

    if first_chars + last_chars >= name.len() {
        return name.to_string();
    }

    format!(
        "{}.{}",
        &name[..first_chars],
        &name[name.len() - last_chars..]
    )
}

/// Gets a color for a tab based on its connection and database
pub fn get_tab_color(tab_name: &str, _index: usize) -> Color {
    // Parse the tab name to extract connection and database info
    let parts: Vec<&str> = tab_name.split(':').collect();

    let connection = parts.get(0).unwrap_or(&"");
    let database = parts.get(1).unwrap_or(&"");

    // Create a hash from connection and database for consistent coloring
    let mut hash = 0u32;
    for byte in connection.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    for byte in database.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }

    // Use the hash to select from a palette of distinguishable colors
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Yellow,
        Color::Magenta,
        Color::Cyan,
        Color::LightRed,
        Color::LightGreen,
        Color::LightBlue,
        Color::LightYellow,
        Color::LightMagenta,
        Color::LightCyan,
    ];

    colors[hash as usize % colors.len()]
}
