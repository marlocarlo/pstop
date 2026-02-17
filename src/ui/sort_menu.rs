use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;
use crate::system::process::ProcessSortField;

/// Draw the sort-by selection menu (F6) — htop-style with arrow-key navigation and scroll
pub fn draw_sort_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(40, 70, f.area());
    f.render_widget(Clear, area);

    let fields = ProcessSortField::all();
    // Available content rows inside border (2 border rows + 2 hint lines + 1 blank)
    let inner_height = area.height.saturating_sub(2) as usize; // minus top+bottom border
    let hint_rows = 2; // blank + hint line
    let visible_items = inner_height.saturating_sub(hint_rows);

    // Calculate scroll offset to keep selected item visible
    let scroll = app.sort_scroll_offset;
    let end = (scroll + visible_items).min(fields.len());

    let mut lines: Vec<Line> = Vec::new();

    for i in scroll..end {
        let field = &fields[i];
        let is_highlighted = i == app.sort_menu_index;
        let is_current = *field == app.sort_field;

        let arrow = if is_current {
            if app.sort_ascending { " ▲" } else { " ▼" }
        } else {
            ""
        };

        let label = format!("  {:<14}{}", field.long_label(), arrow);

        let style = if is_highlighted {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(Span::styled(label, style)));
    }

    lines.push(Line::from(""));
    let scroll_hint = if fields.len() > visible_items {
        format!(" ↑/↓ Navigate  Enter Select  Esc Cancel  [{}/{}]", app.sort_menu_index + 1, fields.len())
    } else {
        " ↑/↓ Navigate  Enter Select  Esc Cancel ".to_string()
    };
    lines.push(Line::from(Span::styled(
        scroll_hint,
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Sort By ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
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
