use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ProcessTab};

/// Draw the tab bar (htop-style: "Main" and "I/O" tabs)
/// Active tab is highlighted with white-on-blue, inactive is dark gray
pub fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    // Background fill
    let bg_fill = " ".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(bg_fill).style(Style::default().bg(Color::Indexed(235))),
        area,
    );

    let active_style = Style::default()
        .fg(Color::White)
        .bg(Color::Blue)
        .add_modifier(Modifier::BOLD);

    let inactive_style = Style::default()
        .fg(Color::DarkGray)
        .bg(Color::Indexed(235));

    let separator_style = Style::default()
        .fg(Color::DarkGray)
        .bg(Color::Indexed(235));

    let (main_style, io_style) = match app.active_tab {
        ProcessTab::Main => (active_style, inactive_style),
        ProcessTab::Io => (inactive_style, active_style),
    };

    let line = Line::from(vec![
        Span::styled(" ", Style::default().bg(Color::Indexed(235))),
        Span::styled(" Main ", main_style),
        Span::styled(" ", separator_style),
        Span::styled(" I/O ", io_style),
        Span::styled("  (Tab to switch)", Style::default().fg(Color::DarkGray).bg(Color::Indexed(235))),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
