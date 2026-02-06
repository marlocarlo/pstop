use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, AppMode};

/// F-key definitions: (key_label, description)
/// htop's exact bottom bar: F1Help F2Setup F3Search F4Filter F5Tree F6SortBy F7Nice- F8Nice+ F9Kill F10Quit
const FKEYS_NORMAL: &[(&str, &str)] = &[
    ("F1", "Help  "),
    ("F2", "Setup "),
    ("F3", "Search"),
    ("F4", "Filter"),
    ("F5", "Tree  "),
    ("F6", "SortBy"),
    ("F7", "Nice -"),
    ("F8", "Nice +"),
    ("F9", "Kill  "),
    ("F10", "Quit "),
];

const FKEYS_SEARCH: &[(&str, &str)] = &[
    ("Esc", "Cancel   "),
    ("", ""),
    ("F3", "Next  "),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("F10", "Quit "),
];

const FKEYS_FILTER: &[(&str, &str)] = &[
    ("Esc", "Clear    "),
    ("Enter", "Accept"),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("", ""),
    ("F10", "Quit "),
];

/// Draw the bottom F-key bar (exact htop styling)
/// htop uses: Fn key in black-on-cyan, description in white-on-black
pub fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    // Full-width dark background first
    let bg_fill = " ".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(bg_fill).style(Style::default().bg(Color::Indexed(234))),
        area,
    );

    let fkeys = match app.mode {
        AppMode::Search => FKEYS_SEARCH,
        AppMode::Filter => FKEYS_FILTER,
        _ => FKEYS_NORMAL,
    };

    let mut spans: Vec<Span> = Vec::new();

    for (key, desc) in fkeys {
        if key.is_empty() {
            continue;
        }
        // Key label: black text on cyan background (htop style)
        spans.push(Span::styled(
            key.to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        // Description: white text on dark background
        spans.push(Span::styled(
            desc.to_string(),
            Style::default()
                .fg(Color::Indexed(252))  // light gray
                .bg(Color::Indexed(234)), // very dark gray
        ));
    }

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line), area);
}
