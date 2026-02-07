use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

/// Draw the Help popup (F1) — comprehensive htop-style help
pub fn draw_help(f: &mut Frame) {
    let area = centered_rect(70, 85, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            " pstop - an htop-like system monitor for Windows ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(" Navigation ", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
        Line::from("  ↑/↓/Alt-k/j Move selection up/down"),
        Line::from("  PgUp/PgDn   Page up/down"),
        Line::from("  Home/End    Jump to first/last process"),
        Line::from(""),
        Line::from(Span::styled(" Function Keys ", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
        Line::from("  F1/h/?      Show this help"),
        Line::from("  F3//        Search (jump to match)"),
        Line::from("  F4/\\        Filter (hide non-matching)"),
        Line::from("  F5/t        Toggle tree view"),
        Line::from("  F6          Open sort menu"),
        Line::from("  F7          Nice - (raise priority)"),
        Line::from("  F8          Nice + (lower priority)"),
        Line::from("  F9/k        Kill process (signal menu)"),
        Line::from("  F10/q       Quit pstop"),
        Line::from(""),
        Line::from(Span::styled(" Sorting ", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
        Line::from("  P           Sort by CPU%"),
        Line::from("  M           Sort by MEM%"),
        Line::from("  T           Sort by TIME"),
        Line::from("  N           Sort by PID"),
        Line::from("  I           Invert sort order"),
        Line::from("  < >         Cycle sort column left/right"),
        Line::from(""),
        Line::from(Span::styled(" Actions ", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))),
        Line::from("  u           Filter by user"),
        Line::from("  a           Set CPU affinity"),
        Line::from("  e           Show process details"),
        Line::from("  F           Follow selected process"),
        Line::from("  Space       Tag/untag process"),
        Line::from("  c           Tag process + all children"),
        Line::from("  U           Untag all processes"),
        Line::from("  H           Toggle show threads"),
        Line::from("  K           Hide kernel/system threads"),
        Line::from("  Z/z         Pause/freeze display"),
        Line::from("  Ctrl+L      Force refresh (unpause)"),
        Line::from("  p           Toggle full command path"),
        Line::from("  +/=         Expand tree node"),
        Line::from("  -           Collapse tree node"),
        Line::from("  *           Expand all tree nodes"),
        Line::from("  0-9         Quick PID search"),
        Line::from("  Ctrl+C      Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press Esc or F1 to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Create a centered rectangle with percentage width/height
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
