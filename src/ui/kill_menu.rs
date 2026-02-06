use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, KILL_SIGNALS};

/// Draw the kill signal selection menu (F9) — like htop's signal chooser
pub fn draw_kill_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(45, 40, f.area());
    f.render_widget(Clear, area);

    let mut lines: Vec<Line> = Vec::new();

    // Title instruction
    lines.push(Line::from(Span::styled(
        " Send signal to selected process: ",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, (sig_num, sig_desc)) in KILL_SIGNALS.iter().enumerate() {
        let is_selected = i == app.kill_signal_index;

        let label = format!("  {:>2}) {}", sig_num, sig_desc);

        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(Span::styled(label, style)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " ↑/↓ Select  Enter Send  Esc Cancel ",
        Style::default().fg(Color::DarkGray),
    )));

    // Show which process will be targeted
    if let Some(proc) = app.selected_process() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" Target: PID {} ({})", proc.pid, proc.name),
            Style::default().fg(Color::Red),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Send Signal ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Red)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black));

    f.render_widget(paragraph, area);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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
