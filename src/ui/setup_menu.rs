use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;
use crate::color_scheme::ColorSchemeId;
use crate::system::process::ProcessSortField;

// ── Constants ───────────────────────────────────────────────────────────────

/// Setup categories (htop: Meters | Display options | Colors | Columns)
const CATEGORIES: &[&str] = &[
    "Meters",
    "Display options",
    "Colors",
    "Columns",
];

/// All display option toggle labels (htop parity)
const DISPLAY_OPTIONS: &[&str] = &[
    "Tree view by default",
    "Shadow other users' processes",
    "Hide kernel threads",
    "Highlight program base name",
    "Highlight large numbers (megabytes)",
    "Display threads in a different color",
    "Leave a margin around header",
    "Detailed CPU time (System/IO-Wait/IRQ)",
    "Count CPUs from zero instead of one",
    "Update process names on every refresh",
    "Show custom thread names",
    "Show full program paths",
    "Show merged command",
    "Enable mouse control",
];

// ── Main draw entry ─────────────────────────────────────────────────────────

pub fn draw_setup_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 88, f.area());
    f.render_widget(Clear, area);

    let cs = &app.color_scheme;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Setup ")
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(cs.popup_border))
        .style(Style::default().bg(cs.popup_bg));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Left 22% categories | Right 78% content
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(22), Constraint::Percentage(78)])
        .split(inner);

    draw_categories_panel(f, app, panels[0]);

    match app.setup_category {
        0 => draw_meters_panel(f, app, panels[1]),
        1 => draw_display_options(f, app, panels[1]),
        2 => draw_colors_panel(f, app, panels[1]),
        3 => draw_columns_panel(f, app, panels[1]),
        _ => {}
    }
}

// ── Category panel (left side) ──────────────────────────────────────────────

fn draw_categories_panel(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let mut lines = vec![
        Line::from(Span::styled(
            " Categories",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (idx, cat) in CATEGORIES.iter().enumerate() {
        let is_active = idx == app.setup_category;
        let is_focused = app.setup_panel == 0;

        let (bg, fg) = if is_active && is_focused {
            (cs.popup_selected_bg, cs.popup_selected_fg)
        } else if is_active {
            (Color::Indexed(236), cs.popup_text)
        } else {
            (Color::Reset, cs.popup_text)
        };

        lines.push(Line::from(Span::styled(
            format!(" {:<18}", cat),
            Style::default().fg(fg).bg(bg),
        )));
    }

    // Controls hint
    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " ←→ Panel",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        " ↑↓ Navigate",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        " Esc Close",
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(lines), area);
}

// ── Meters panel (category 0) ───────────────────────────────────────────────

fn draw_meters_panel(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;

    // Split: Left column meters | Right column meters | Available meters
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(area);

    // Left column meters (from app state)
    let mut left_lines = vec![
        Line::from(Span::styled(
            " Left Column",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
    ];
    for (i, m) in app.left_meters.iter().enumerate() {
        let is_sel = app.setup_panel == 1 && app.setup_meter_col == 0 && i == app.setup_menu_index;
        let bg = if is_sel { Color::Indexed(236) } else { Color::Reset };
        let fg = if is_sel { Color::Yellow } else { cs.popup_text };
        left_lines.push(Line::from(Span::styled(
            format!("  {}", m),
            Style::default().fg(fg).bg(bg),
        )));
    }
    f.render_widget(Paragraph::new(left_lines), cols[0]);

    // Right column meters (from app state)
    let mut right_lines = vec![
        Line::from(Span::styled(
            " Right Column",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
    ];
    for (i, m) in app.right_meters.iter().enumerate() {
        let is_sel = app.setup_panel == 1 && app.setup_meter_col == 1 && i == app.setup_menu_index;
        let bg = if is_sel { Color::Indexed(236) } else { Color::Reset };
        let fg = if is_sel { Color::Yellow } else { cs.popup_text };
        right_lines.push(Line::from(Span::styled(
            format!("  {}", m),
            Style::default().fg(fg).bg(bg),
        )));
    }
    f.render_widget(Paragraph::new(right_lines), cols[1]);

    // Available meters
    let available = vec![
        "CPU average", "CPU (1/1) [Bar]", "CPU (1/1) [Text]",
        "CPU (1/1) [Graph]", "CPU (1/1) [LED]",
        "Memory [Bar]", "Memory [Text]",
        "Swap [Bar]", "Swap [Text]",
        "Network [Bar]", "Clock", "Hostname",
        "Uptime", "Battery", "Tasks", "Load average",
        "Blank",
    ];
    let mut avail_lines = vec![
        Line::from(Span::styled(
            " Available Meters",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
    ];
    for meter in &available {
        avail_lines.push(Line::from(Span::styled(
            format!("  {}", meter),
            Style::default().fg(Color::DarkGray),
        )));
    }
    avail_lines.push(Line::from(""));
    avail_lines.push(Line::from(Span::styled(
        "  ←→ Column  ↑↓ Navigate",
        Style::default().fg(Color::DarkGray),
    )));
    avail_lines.push(Line::from(Span::styled(
        "  Enter Add  Del Remove",
        Style::default().fg(Color::DarkGray),
    )));
    avail_lines.push(Line::from(Span::styled(
        "  F7 Move up  F8 Move down",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(Paragraph::new(avail_lines), cols[2]);
}

// ── Display options panel (category 1) ──────────────────────────────────────

fn draw_display_options(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;

    let mut lines = vec![
        Line::from(Span::styled(
            " Display options",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let toggle_values = [
        app.show_tree_by_default,
        app.shadow_other_users,
        app.hide_kernel_threads,
        app.highlight_base_name,
        app.highlight_megabytes,
        app.highlight_threads,
        app.header_margin,
        app.detailed_cpu_time,
        app.cpu_count_from_zero,
        app.update_process_names,
        app.show_thread_names,
        app.show_full_path,
        app.show_merged_command,
        app.enable_mouse,
    ];

    for (idx, (label, &value)) in DISPLAY_OPTIONS.iter().zip(toggle_values.iter()).enumerate() {
        let is_selected = app.setup_panel == 1 && idx == app.setup_menu_index;

        let checkbox = if value { "[X]" } else { "[ ]" };
        let check_color = if value { Color::Green } else { Color::DarkGray };

        let bg = if is_selected { Color::Indexed(236) } else { Color::Reset };
        let fg = if is_selected { Color::Yellow } else { cs.popup_text };

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().bg(bg)),
            Span::styled(
                format!("{} ", checkbox),
                Style::default().fg(check_color).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<42}", label),
                Style::default().fg(fg).bg(bg),
            ),
        ]));
    }

    lines.push(Line::from(""));

    // Update interval
    let interval_sel = app.setup_panel == 1 && app.setup_menu_index == DISPLAY_OPTIONS.len();
    let interval_bg = if interval_sel { Color::Indexed(236) } else { Color::Reset };
    let interval_fg = if interval_sel { Color::Yellow } else { cs.popup_text };
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default().bg(interval_bg)),
        Span::styled(
            format!("Update interval:  {} ms", app.update_interval_ms),
            Style::default().fg(interval_fg).bg(interval_bg),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "     (+/- to adjust, 200–10000 ms)",
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Space/Enter=toggle  ↑↓=navigate  +/-=interval",
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(lines), area);
}

// ── Colors panel (category 2) ───────────────────────────────────────────────

fn draw_colors_panel(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;

    // Split: scheme list (left 35%) | preview (right 65%)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Scheme list
    let mut list_lines = vec![
        Line::from(Span::styled(
            " Color Schemes",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (idx, scheme_id) in ColorSchemeId::all().iter().enumerate() {
        let is_current = *scheme_id == app.color_scheme_id;
        let is_selected = app.setup_panel == 1 && idx == app.setup_menu_index;

        let prefix = if is_current { "● " } else { "  " };
        let bg = if is_selected { Color::Indexed(236) } else { Color::Reset };
        let fg = if is_selected {
            Color::Yellow
        } else if is_current {
            Color::Green
        } else {
            cs.popup_text
        };

        list_lines.push(Line::from(vec![
            Span::styled(
                prefix,
                Style::default().fg(if is_current { Color::Green } else { Color::DarkGray }).bg(bg),
            ),
            Span::styled(
                format!("{:<18}", scheme_id.name()),
                Style::default().fg(fg).bg(bg).add_modifier(
                    if is_current { Modifier::BOLD } else { Modifier::empty() }
                ),
            ),
        ]));
    }

    list_lines.push(Line::from(""));
    list_lines.push(Line::from(Span::styled(
        "  Enter=apply  ↑↓=browse",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(Paragraph::new(list_lines), cols[0]);

    // Preview panel — show what the selected scheme looks like
    let preview_idx = if app.setup_panel == 1 { app.setup_menu_index } else { app.color_scheme_id as usize };
    let preview_id = ColorSchemeId::from_index(preview_idx);
    let preview = crate::color_scheme::ColorScheme::from_id(preview_id);

    let mut prev_lines = vec![
        Line::from(Span::styled(
            " Preview",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" Scheme: {}", preview_id.name()),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!(" {}", preview_id.description()),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    // CPU bar preview
    prev_lines.push(Line::from(vec![
        Span::styled(" 0", Style::default().fg(preview.cpu_label)),
        Span::styled("[", Style::default().fg(preview.cpu_label)),
        Span::styled("||||", Style::default().fg(preview.cpu_bar_low)),
        Span::styled("||||||", Style::default().fg(preview.cpu_bar_normal)),
        Span::styled("|||", Style::default().fg(preview.cpu_bar_system)),
        Span::styled("       ", Style::default().fg(preview.cpu_bar_bg)),
        Span::styled(" 42.1%]", Style::default().fg(preview.cpu_label)),
    ]));

    // Mem bar preview
    prev_lines.push(Line::from(vec![
        Span::styled(" Mem", Style::default().fg(preview.cpu_label)),
        Span::styled("[", Style::default().fg(preview.cpu_label)),
        Span::styled("|||||||", Style::default().fg(preview.mem_bar_used)),
        Span::styled("|||", Style::default().fg(preview.mem_bar_buffers)),
        Span::styled("||", Style::default().fg(preview.mem_bar_cache)),
        Span::styled("     ", Style::default().fg(preview.cpu_bar_bg)),
        Span::styled(" 8.2G/16G]", Style::default().fg(preview.cpu_label)),
    ]));

    // Swap bar preview
    prev_lines.push(Line::from(vec![
        Span::styled(" Swp", Style::default().fg(preview.cpu_label)),
        Span::styled("[", Style::default().fg(preview.cpu_label)),
        Span::styled("|||", Style::default().fg(preview.swap_bar)),
        Span::styled("                ", Style::default().fg(preview.cpu_bar_bg)),
        Span::styled(" 1.5G/8G]", Style::default().fg(preview.cpu_label)),
    ]));

    prev_lines.push(Line::from(""));

    // Header preview
    prev_lines.push(Line::from(vec![
        Span::styled(
            " PID    USER     CPU%  MEM%  COMMAND",
            Style::default().fg(preview.table_header_fg).bg(preview.table_header_bg),
        ),
    ]));

    // Process row previews
    let preview_procs = [
        ("  1234 ", "root    ", " 42.1", "  3.2", " /usr/bin/python"),
        ("  5678 ", "admin   ", "  8.3", "  1.1", " firefox"),
        ("   910 ", "nobody  ", "  0.1", "  0.0", " sshd"),
    ];
    for (i, (pid, user, cpu, mem, cmd)) in preview_procs.iter().enumerate() {
        let bg = if i == 0 { preview.process_selected_bg } else { preview.process_bg };
        let fg = if i == 0 { preview.process_selected_fg } else { preview.process_fg };
        prev_lines.push(Line::from(vec![
            Span::styled(*pid, Style::default().fg(preview.col_pid).bg(bg)),
            Span::styled(*user, Style::default().fg(if i == 2 { preview.process_shadow } else { preview.col_user }).bg(bg)),
            Span::styled(*cpu, Style::default().fg(if i == 0 { preview.col_cpu_high } else { preview.col_cpu_low }).bg(bg)),
            Span::styled(*mem, Style::default().fg(fg).bg(bg)),
            Span::styled(*cmd, Style::default().fg(preview.col_command_basename).bg(bg)),
        ]));
    }

    prev_lines.push(Line::from(""));

    // Footer preview
    prev_lines.push(Line::from(vec![
        Span::styled("F1", Style::default().fg(preview.footer_key_fg).bg(preview.footer_key_bg)),
        Span::styled("Help ", Style::default().fg(preview.footer_label_fg).bg(preview.footer_label_bg)),
        Span::styled("F2", Style::default().fg(preview.footer_key_fg).bg(preview.footer_key_bg)),
        Span::styled("Setup", Style::default().fg(preview.footer_label_fg).bg(preview.footer_label_bg)),
    ]));

    f.render_widget(Paragraph::new(prev_lines), cols[1]);
}

// ── Columns panel (category 3) ──────────────────────────────────────────────

fn draw_columns_panel(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let all_fields = ProcessSortField::all();

    // Split: Column list (left) | Description (right)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // All columns with toggle checkmarks
    let mut col_lines = vec![
        Line::from(Span::styled(
            " Columns",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (idx, field) in all_fields.iter().enumerate() {
        let is_active = app.visible_columns.contains(field);
        let is_selected = app.setup_panel == 1 && idx == app.setup_menu_index;

        let checkbox = if is_active { "[X]" } else { "[ ]" };
        let check_color = if is_active { Color::Green } else { Color::DarkGray };
        let bg = if is_selected { Color::Indexed(236) } else { Color::Reset };
        let fg = if is_selected { Color::Yellow } else if is_active { Color::Green } else { cs.popup_text };

        col_lines.push(Line::from(vec![
            Span::styled("  ", Style::default().bg(bg)),
            Span::styled(
                format!("{} ", checkbox),
                Style::default().fg(check_color).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<14}", field.long_label()),
                Style::default().fg(fg).bg(bg),
            ),
        ]));
    }

    col_lines.push(Line::from(""));
    col_lines.push(Line::from(Span::styled(
        "  Space=toggle  a=toggle all",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(Paragraph::new(col_lines), cols[0]);

    // Description panel for currently selected column
    let mut desc_lines = vec![
        Line::from(Span::styled(
            " Column Details",
            Style::default().fg(cs.popup_title).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(field) = all_fields.get(app.setup_menu_index) {
        let is_active = app.visible_columns.contains(field);
        let desc = field_description(field);
        desc_lines.push(Line::from(Span::styled(
            format!("  {}", field.long_label()),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        desc_lines.push(Line::from(Span::styled(
            format!("  Short: {}", field.label()),
            Style::default().fg(Color::DarkGray),
        )));
        desc_lines.push(Line::from(Span::styled(
            format!("  {}", desc),
            Style::default().fg(cs.popup_text),
        )));
        desc_lines.push(Line::from(""));
        let status = if is_active { "Visible" } else { "Hidden" };
        let status_color = if is_active { Color::Green } else { Color::DarkGray };
        desc_lines.push(Line::from(Span::styled(
            format!("  Status: {}", status),
            Style::default().fg(status_color),
        )));
    }

    f.render_widget(Paragraph::new(desc_lines), cols[1]);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn field_description(field: &ProcessSortField) -> &'static str {
    match field {
        ProcessSortField::Pid => "Process/thread ID",
        ProcessSortField::Ppid => "Parent process ID",
        ProcessSortField::User => "Owner username",
        ProcessSortField::Priority => "Kernel scheduling priority",
        ProcessSortField::Nice => "Nice value (higher=more yielding)",
        ProcessSortField::VirtMem => "Total virtual memory size",
        ProcessSortField::ResMem => "Resident set size (physical RAM)",
        ProcessSortField::SharedMem => "Shared memory pages size",
        ProcessSortField::Status => "State (S/R/D/T/Z)",
        ProcessSortField::Cpu => "Percentage of CPU time",
        ProcessSortField::Mem => "Percentage of physical memory",
        ProcessSortField::Time => "Total CPU time consumed",
        ProcessSortField::Threads => "Thread count (NLWP)",
        ProcessSortField::Command => "Process command line",
        ProcessSortField::IoReadRate => "Disk read bytes/sec",
        ProcessSortField::IoWriteRate => "Disk write bytes/sec",
        ProcessSortField::IoRate => "Combined read+write I/O rate",
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
