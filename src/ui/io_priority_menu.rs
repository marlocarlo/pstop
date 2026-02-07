use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

/// Draw I/O priority selection menu (htop 'i' key)
pub fn draw_io_priority_menu(f: &mut Frame, app: &App) {
    let proc = match app.selected_process() {
        Some(p) => p,
        None => return,
    };

    let area = centered_rect(50, 30, f.area());
    f.render_widget(Clear, area);

    let options = vec![
        "Normal I/O Priority",
        "Background Mode (Low I/O)",
    ];

    let selected_idx = app.io_priority_index;

    let mut lines = vec![
        Line::from(Span::styled(
            format!(" Set I/O Priority - {} (PID: {}) ", proc.name, proc.pid),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (i, option) in options.iter().enumerate() {
        let style = if i == selected_idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(format!("  {}", option), style)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Use ↑/↓ to select, Enter to apply, Esc to cancel ",
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Note: Background mode reduces both I/O and CPU priority ",
        Style::default().fg(Color::Yellow),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" I/O Priority ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black));

    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    use ratatui::layout::{Direction, Layout};
    
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
