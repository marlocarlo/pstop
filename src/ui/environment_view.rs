use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::App;
use crate::system::memory::format_bytes;

/// Draw the Environment/Process Details viewer (htop 'e')
pub fn draw_environment_view(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 85, f.area());
    f.render_widget(Clear, area);

    let proc = match app.selected_process() {
        Some(p) => p,
        None => return,
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!(" Process Details - PID {} ", proc.pid),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Process information
    lines.extend(vec![
        Line::from(vec![
            Span::styled("Name:         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&proc.name, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("PID:          ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}", proc.pid), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Parent PID:   ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}", proc.ppid), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("User:         ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&proc.user, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Status:       ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}", proc.status.symbol()), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Priority:     ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} (Nice: {})", proc.priority, proc.nice), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Threads:      ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}", proc.threads), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Memory Usage ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("Virtual:      ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format_bytes(proc.virtual_mem), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Resident:     ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format_bytes(proc.resident_mem), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Shared:       ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format_bytes(proc.shared_mem), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Memory %:     ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.1}%", proc.mem_usage), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Performance ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("CPU %:        ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.1}%", proc.cpu_usage), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Runtime:      ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(proc.format_time(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("I/O Read:     ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format_io_rate(proc.io_read_rate), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("I/O Write:    ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format_io_rate(proc.io_write_rate), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Command Line ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(&proc.command, Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            " Press Esc, e, or q to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ]);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Process Details ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn format_io_rate(rate: f64) -> String {
    if rate == 0.0 {
        "0 B/s".to_string()
    } else if rate < 1024.0 {
        format!("{} B/s", rate as u64)
    } else if rate < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", rate / 1024.0)
    } else if rate < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB/s", rate / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB/s", rate / (1024.0 * 1024.0 * 1024.0))
    }
}

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
