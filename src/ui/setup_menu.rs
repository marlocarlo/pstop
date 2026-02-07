use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;
use crate::system::process::ProcessSortField;

/// Draw the F2 Setup menu (htop column configuration)
pub fn draw_setup_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 80, f.area());
    f.render_widget(Clear, area);

    let all_fields = ProcessSortField::all();
    
    let mut lines = vec![
        Line::from(Span::styled(
            " Setup - Column Configuration  (F2) ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Select which columns to display:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    // List all available columns with checkboxes
    for (idx, field) in all_fields.iter().enumerate() {
        let is_selected = idx == app.setup_menu_index;
        let is_visible = app.visible_columns.contains(field);
        
        let checkbox = if is_visible { "[X]" } else { "[ ]" };
        let check_color = if is_visible { Color::Green } else { Color::DarkGray };
        
        let bg = if is_selected { Color::Indexed(236) } else { Color::Reset };
        let fg = if is_selected { Color::Yellow } else { Color::White };
        
        let label = field.long_label();
        
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().bg(bg)),
            Span::styled(
                format!("{} ", checkbox),
                Style::default().fg(check_color).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<20}", label),
                Style::default().fg(fg).bg(bg),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Controls:",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from("  ↑/↓       Navigate columns"));
    lines.push(Line::from("  Space     Toggle column visibility"));
    lines.push(Line::from("  a         Toggle all columns"));
    lines.push(Line::from("  Esc/F2    Close setup menu"));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Note: Command column cannot be hidden",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Setup ")
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
