use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ProcessTab};

/// Draw the tab bar (htop-style: "Main" and "I/O" tabs)
/// Active tab is highlighted with white-on-blue, inactive is dark gray
pub fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    // Background fill
    let bg_fill = " ".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(bg_fill).style(Style::default().bg(cs.tab_inactive_bg)),
        area,
    );

    let active_style = Style::default()
        .fg(cs.tab_active_fg)
        .bg(cs.tab_active_bg)
        .add_modifier(Modifier::BOLD);

    let inactive_style = Style::default()
        .fg(cs.tab_inactive_fg)
        .bg(cs.tab_inactive_bg);

    let separator_style = Style::default()
        .fg(cs.tab_inactive_fg)
        .bg(cs.tab_inactive_bg);

    let (main_style, io_style, net_style, gpu_style) = match app.active_tab {
        ProcessTab::Main => (active_style, inactive_style, inactive_style, inactive_style),
        ProcessTab::Io => (inactive_style, active_style, inactive_style, inactive_style),
        ProcessTab::Net => (inactive_style, inactive_style, active_style, inactive_style),
        ProcessTab::Gpu => (inactive_style, inactive_style, inactive_style, active_style),
    };

    let line = Line::from(vec![
        Span::styled(" ", Style::default().bg(cs.tab_inactive_bg)),
        Span::styled(" Main ", main_style),
        Span::styled(" ", separator_style),
        Span::styled(" I/O ", io_style),
        Span::styled(" ", separator_style),
        Span::styled(" Net ", net_style),
        Span::styled(" ", separator_style),
        Span::styled(" GPU ", gpu_style),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
