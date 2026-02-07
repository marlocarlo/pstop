use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::App;

/// Draw open files/handles viewer (htop 'l' key - lsof equivalent)
/// Shows handles opened by the selected process
pub fn draw_handles_view(f: &mut Frame, app: &App) {
    let proc = match app.selected_process() {
        Some(p) => p,
        None => return,
    };

    let area = centered_rect(80, 80, f.area());
    f.render_widget(Clear, area);

    // Get handle information for this process
    let handle_info = crate::system::winapi::get_process_handles(proc.pid);
    
    let mut lines = vec![
        Line::from(Span::styled(
            format!(" Open Files/Handles - {} (PID: {}) ", proc.name, proc.pid),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if handle_info.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Unable to enumerate handles for this process",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  (Requires elevation for most processes)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  Total Handles: {}", handle_info.len()),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  TYPE       PATH",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "  ─────────  ───────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));

        // Show first 100 handles (to avoid overwhelming the display)
        for handle in handle_info.iter().take(100) {
            let type_str = format!("{:<10}", truncate_str(&handle.handle_type, 10));
            let path_str = truncate_str(&handle.name, 70);
            
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(type_str, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(path_str, Style::default().fg(Color::White)),
            ]));
        }

        if handle_info.len() > 100 {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  ... and {} more handles", handle_info.len() - 100),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Press Esc or l to close ",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Open Files/Handles (lsof) ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let mut truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    } else {
        s.to_string()
    }
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
