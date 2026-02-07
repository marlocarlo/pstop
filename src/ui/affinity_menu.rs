use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

/// Draw the CPU Affinity selector (htop 'a')
pub fn draw_affinity_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, f.area());
    f.render_widget(Clear, area);

    let proc = match app.selected_process() {
        Some(p) => p,
        None => return,
    };

    let cpu_count = app.affinity_cpus.len();
    
    let mut lines = vec![
        Line::from(Span::styled(
            format!(" CPU Affinity for PID {} - {} ", proc.pid, proc.name),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Select which CPU cores this process can run on:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    // Display CPUs in a grid layout (2 columns if many CPUs)
    for i in 0..cpu_count {
        let checkbox = if app.affinity_cpus[i] { "[X]" } else { "[ ]" };
        let color = if app.affinity_cpus[i] { Color::Green } else { Color::DarkGray };
        
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{} ", checkbox), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("CPU {}", i), Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Controls:",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from("  0-9    Toggle specific CPU"));
    lines.push(Line::from("  Space  Toggle CPU 0"));
    lines.push(Line::from("  a      Toggle all CPUs"));
    lines.push(Line::from("  Enter  Apply and close"));
    lines.push(Line::from("  Esc    Cancel"));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" CPU Affinity ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black));

    f.render_widget(paragraph, area);
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
