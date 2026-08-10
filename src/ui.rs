//! Terminal UI rendering.

use crate::app::{App, Mode, Selection};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

/// Draws the entire application UI into the given frame.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Length(4), // package manager menu
            Constraint::Min(6),    // results table
            Constraint::Length(3), // fastest mirror
            Constraint::Length(3), // status / help
        ])
        .split(area);

    draw_title(frame, chunks[0]);
    draw_menu(frame, chunks[1], app);
    draw_results(frame, chunks[2], app);
    draw_fastest(frame, chunks[3], app);

    if app.mode == Mode::Input {
        draw_input(frame, chunks[4], app);
    } else {
        draw_status(frame, chunks[4], app);
    }
}

fn draw_title(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new("Mirror Benchmark")
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, area);
}

fn draw_menu(frame: &mut Frame, area: Rect, app: &App) {
    let selected_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let pypi_prefix = if app.selection == Selection::PyPi {
        "> "
    } else {
        "  "
    };
    let npm_prefix = if app.selection == Selection::Npm {
        "> "
    } else {
        "  "
    };

    let pypi_style = if app.selection == Selection::PyPi {
        selected_style
    } else {
        Style::default()
    };
    let npm_style = if app.selection == Selection::Npm {
        selected_style
    } else {
        Style::default()
    };

    let lines = vec![
        Line::from(Span::styled(format!("{pypi_prefix}PyPI"), pypi_style)),
        Line::from(Span::styled(format!("{npm_prefix}npm"), npm_style)),
    ];

    let menu = Paragraph::new(lines).block(
        Block::default()
            .title("Package Manager")
            .borders(Borders::ALL),
    );
    frame.render_widget(menu, area);
}

fn draw_results(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("Mirror"),
        Cell::from("Avg(ms)"),
        Cell::from("Success"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = app
        .results
        .iter()
        .map(|r| {
            let latency = if r.timed_out {
                "timeout".to_string()
            } else {
                r.average_latency_ms.to_string()
            };
            let success = format!("{:.0}%", r.success_rate);

            let style = if r.timed_out {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };

            Row::new(vec![
                Cell::from(r.name.clone()),
                Cell::from(latency),
                Cell::from(success),
            ])
            .style(style)
        })
        .collect();

    if app.running {
        if let Some(config) = &app.config {
            if app.benchmark_index < config.mirrors.len() {
                let pending = &config.mirrors[app.benchmark_index];
                let testing_style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD);
                rows.push(
                    Row::new(vec![
                        Cell::from(format!("(testing {})", pending)),
                        Cell::from(""),
                        Cell::from(""),
                    ])
                    .style(testing_style),
                );
            }
        }
    }

    let widths = [
        Constraint::Percentage(60),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title("Results").borders(Borders::ALL));

    frame.render_widget(table, area);
}

fn draw_fastest(frame: &mut Frame, area: Rect, app: &App) {
    let text = app.fastest().unwrap_or("N/A");
    let widget = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .title("Fastest Mirror")
                .borders(Borders::ALL),
        );
    frame.render_widget(widget, area);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let help = format!(
        "{}   [q] Quit   [Enter] Run Benchmark   [up/down] Switch   [a] Add Mirror",
        app.status
    );
    let widget = Paragraph::new(help).block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

fn draw_input(frame: &mut Frame, area: Rect, app: &App) {
    let widget = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .title("Add Mirror URL  [Enter] Save  [Esc] Cancel")
                .borders(Borders::ALL),
        );
    frame.render_widget(widget, area);

    frame.set_cursor_position((area.x + 1 + app.cursor as u16, area.y + 1));
}
