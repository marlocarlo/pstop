use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
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
/// htop evenly distributes 10 F-key buttons across the full terminal width.
/// Each button: Fn key in black-on-cyan, label in black-on-blue (default scheme).
pub fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    // Full-width background fill
    let bg_fill = " ".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(bg_fill).style(Style::default().bg(cs.footer_label_bg)),
        area,
    );

    let fkeys = match app.mode {
        AppMode::Search => FKEYS_SEARCH,
        AppMode::Filter => FKEYS_FILTER,
        _ => FKEYS_NORMAL,
    };

    // Collect active (non-empty) entries
    let active: Vec<(&str, &str)> = fkeys.iter()
        .filter(|(k, _)| !k.is_empty())
        .copied()
        .collect();

    let total_width = area.width as usize;
    let n = active.len();
    if n == 0 {
        return;
    }

    // htop distributes items evenly: each slot = total_width / n
    let slot_width = total_width / n;

    let mut spans: Vec<Span> = Vec::new();

    for (i, (key, desc)) in active.iter().enumerate() {
        let key_str = key.to_string();
        let desc_trimmed = desc.trim_end();

        // Remaining space in this slot for the label
        let label_width = slot_width.saturating_sub(key_str.len());
        // Last slot gets any remaining width
        let label_width = if i == n - 1 {
            total_width.saturating_sub(slot_width * (n - 1)).saturating_sub(key_str.len())
        } else {
            label_width
        };

        // Pad the label to fill its portion of the slot
        let padded_desc = format!("{:<width$}", desc_trimmed, width = label_width);

        // Key label: styled per color scheme
        spans.push(Span::styled(
            key_str,
            Style::default()
                .fg(cs.footer_key_fg)
                .bg(cs.footer_key_bg)
                .add_modifier(Modifier::BOLD),
        ));
        // Description: styled per color scheme
        spans.push(Span::styled(
            padded_desc,
            Style::default()
                .fg(cs.footer_label_fg)
                .bg(cs.footer_label_bg),
        ));
    }

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line), area);
}
