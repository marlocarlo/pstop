use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, AppMode, ProcessTab};
use crate::system::memory::format_bytes;
use crate::system::process::ProcessSortField;

/// htop's exact default column headers and widths:
/// PID USER PRI NI VIRT RES SHR S CPU% MEM% TIME+ Command
/// Note: I/O columns are shown when available (optional in htop via F2 setup)
const HEADERS: &[(&str, u16, ProcessSortField)] = &[
    ("PID",        7,  ProcessSortField::Pid),
    ("PPID",       7,  ProcessSortField::Ppid),
    ("USER",       9,  ProcessSortField::User),
    ("PRI",        4,  ProcessSortField::Priority),
    ("NI",         4,  ProcessSortField::Nice),
    ("VIRT",       7,  ProcessSortField::VirtMem),
    ("RES",        7,  ProcessSortField::ResMem),
    ("SHR",        7,  ProcessSortField::SharedMem),
    ("S",          2,  ProcessSortField::Status),
    ("CPU%",       6,  ProcessSortField::Cpu),
    ("MEM%",       6,  ProcessSortField::Mem),
    ("TIME+",     10,  ProcessSortField::Time),
    ("THR",        4,  ProcessSortField::Threads),
    ("IO_R",      10,  ProcessSortField::IoReadRate),   // htop: DISK READ
    ("IO_W",      10,  ProcessSortField::IoWriteRate),  // htop: DISK WRITE
    ("Command",    0,  ProcessSortField::Command), // 0 = takes remaining space
];

/// htop I/O tab column headers
/// PID USER IO DISK R/W DISK READ DISK WRITE SWPD% IOD% Command
const IO_HEADERS: &[(&str, u16, ProcessSortField)] = &[
    ("PID",         7,  ProcessSortField::Pid),
    ("USER",        9,  ProcessSortField::User),
    ("IO",          4,  ProcessSortField::Priority),    // I/O priority (maps from process priority)
    ("DISK R/Mv",  10,  ProcessSortField::IoRate),      // Combined read+write
    ("DISK READ",  10,  ProcessSortField::IoReadRate),
    ("DISK WRITE", 11,  ProcessSortField::IoWriteRate),
    ("SWPD%",       6,  ProcessSortField::Mem),         // Swap percentage approximation
    ("IOD%",        6,  ProcessSortField::Cpu),         // I/O delay (approximated)
    ("Command",     0,  ProcessSortField::Command),
];

/// Network tab column headers (pstop extension)
/// PID USER  S  CPU%  IO_READ  IO_WRITE  TOTAL_IO  Command
const NET_HEADERS: &[(&str, u16, ProcessSortField)] = &[
    ("PID",        7,  ProcessSortField::Pid),
    ("USER",       9,  ProcessSortField::User),
    ("S",          2,  ProcessSortField::Status),
    ("CPU%",       6,  ProcessSortField::Cpu),
    ("IO READ",   10,  ProcessSortField::IoReadRate),
    ("IO WRITE",  10,  ProcessSortField::IoWriteRate),
    ("TOTAL IO",  10,  ProcessSortField::IoRate),
    ("MEM%",       6,  ProcessSortField::Mem),
    ("Command",    0,  ProcessSortField::Command),
];

/// Draw the process table
pub fn draw_process_table(f: &mut Frame, app: &App, area: Rect) {
    if area.height < 2 {
        return;
    }

    // Select headers based on active tab
    let headers = match app.active_tab {
        ProcessTab::Main => HEADERS,
        ProcessTab::Io => IO_HEADERS,
        ProcessTab::Net => NET_HEADERS,
    };

    // --- Column header row (full-width colored background like htop) ---
    let header_area = Rect { x: area.x, y: area.y, width: area.width, height: 1 };

    // Build a full-width background for the header
    let cs = &app.color_scheme;
    let bg_line = " ".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(bg_line).style(Style::default().bg(cs.table_header_bg).fg(cs.table_header_fg)),
        header_area,
    );

    // Build header spans with sort indicator
    let mut header_spans: Vec<Span> = Vec::new();
    for (name, width, sort_field) in headers {
        // On Main tab, skip columns that are not visible (F2 setup menu)
        if app.active_tab == ProcessTab::Main && !app.visible_columns.contains(sort_field) {
            continue;
        }
        
        let is_sorted = *sort_field == app.sort_field;
        let fixed_w = match app.active_tab {
            ProcessTab::Main => fixed_cols_width_visible(app),
            ProcessTab::Io => io_fixed_cols_width(),
            ProcessTab::Net => net_fixed_cols_width(),
        };
        let w = if *width == 0 { (area.width as usize).saturating_sub(fixed_w) } else { *width as usize };

        let display = if is_sorted {
            let arrow = if app.sort_ascending { "▲" } else { "▼" };
            format!("{}{}", name, arrow)
        } else {
            name.to_string()
        };

        let padded = if *width == 0 {
            display // Command column: no padding
        } else {
            format!("{:<width$}", display, width = w)
        };

        let style = if is_sorted {
            Style::default().fg(cs.table_header_sort_fg).bg(cs.table_header_sort_bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(cs.table_header_fg).bg(cs.table_header_bg)
        };

        header_spans.push(Span::styled(padded, style));
    }
    let header_line = Line::from(header_spans);
    f.render_widget(Paragraph::new(header_line), header_area);

    // --- Process rows ---
    let table_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height - 1,
    };

    // Search bar takes 1 row at bottom if active
    let (proc_area, bar_area) = if app.mode == AppMode::Search || app.mode == AppMode::Filter {
        let proc_h = table_area.height.saturating_sub(1);
        (
            Rect { height: proc_h, ..table_area },
            Some(Rect {
                x: table_area.x,
                y: table_area.y + proc_h,
                width: table_area.width,
                height: 1,
            }),
        )
    } else if !app.filter_query.is_empty() {
        // Filter is active even in Normal mode — show persistent indicator
        let proc_h = table_area.height.saturating_sub(1);
        (
            Rect { height: proc_h, ..table_area },
            Some(Rect {
                x: table_area.x,
                y: table_area.y + proc_h,
                width: table_area.width,
                height: 1,
            }),
        )
    } else {
        (table_area, None)
    };

    let visible = proc_area.height as usize;
    let start = app.scroll_offset;
    let end = (start + visible).min(app.filtered_processes.len());

    for (i, row_idx) in (start..end).enumerate() {
        let proc = &app.filtered_processes[row_idx];
        let is_selected = row_idx == app.selected_index;
        let is_tagged = app.tagged_pids.contains(&proc.pid);

        let row_area = Rect {
            x: proc_area.x,
            y: proc_area.y + i as u16,
            width: proc_area.width,
            height: 1,
        };

        let row_line = match app.active_tab {
            ProcessTab::Main => build_process_row(proc, row_area.width as usize, app, is_selected, is_tagged),
            ProcessTab::Io => build_io_row(proc, row_area.width as usize, app, is_selected, is_tagged),
            ProcessTab::Net => build_net_row(proc, row_area.width as usize, app, is_selected, is_tagged),
        };
        f.render_widget(Paragraph::new(row_line), row_area);
    }

    // Search / Filter bar
    if let Some(bar_rect) = bar_area {
        let bar_line = if app.mode == AppMode::Search {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(cs.search_label).add_modifier(Modifier::BOLD)),
                Span::styled(app.search_query.clone(), Style::default().fg(cs.search_text)),
                Span::styled("_", Style::default().fg(cs.search_text).add_modifier(Modifier::SLOW_BLINK)),
            ])
        } else if app.mode == AppMode::Filter {
            Line::from(vec![
                Span::styled("Filter: ", Style::default().fg(cs.filter_label).add_modifier(Modifier::BOLD)),
                Span::styled(app.filter_query.clone(), Style::default().fg(cs.filter_text)),
                Span::styled("_", Style::default().fg(cs.filter_text).add_modifier(Modifier::SLOW_BLINK)),
            ])
        } else {
            Line::from(vec![
                Span::styled("Filter[active]: ", Style::default().fg(cs.filter_label).add_modifier(Modifier::BOLD)),
                Span::styled(app.filter_query.clone(), Style::default().fg(cs.filter_text)),
            ])
        };
        f.render_widget(Paragraph::new(bar_line), bar_rect);
    }
}

/// Total width of all fixed-width columns (for calculating Command column)
fn fixed_cols_width() -> usize {
    HEADERS.iter().map(|(_, w, _)| if *w > 0 { *w as usize + 1 } else { 0 }).sum()
}

/// Total width of visible fixed-width columns
fn fixed_cols_width_visible(app: &App) -> usize {
    HEADERS.iter()
        .filter(|(_, _, field)| app.visible_columns.contains(field))
        .map(|(_, w, _)| if *w > 0 { *w as usize + 1 } else { 0 })
        .sum()
}

/// Total width of I/O tab fixed-width columns
fn io_fixed_cols_width() -> usize {
    IO_HEADERS.iter().map(|(_, w, _)| if *w > 0 { *w as usize + 1 } else { 0 }).sum()
}

/// Total width of Net tab fixed-width columns
fn net_fixed_cols_width() -> usize {
    NET_HEADERS.iter().map(|(_, w, _)| if *w > 0 { *w as usize + 1 } else { 0 }).sum()
}

/// Build a single process row as a styled Line (matching htop's exact columns)
fn build_process_row(
    proc: &crate::system::process::ProcessInfo,
    width: usize,
    app: &App,
    selected: bool,
    tagged: bool,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };
    let default_fg = if selected { cs.process_selected_fg } else { cs.process_fg };

    let pid_fg = if tagged { Color::Yellow } else { cs.col_pid };

    let cpu_fg = if proc.cpu_usage > 90.0 { cs.col_cpu_high }
        else if proc.cpu_usage > 50.0 { cs.col_cpu_medium }
        else { cs.col_cpu_low };

    let mem_fg = if proc.mem_usage > 50.0 { cs.col_mem_high }
        else if proc.mem_usage > 20.0 { cs.col_cpu_medium }
        else { cs.col_mem_normal };

    let status_fg = match &proc.status {
        crate::system::process::ProcessStatus::Running => cs.col_status_running,
        crate::system::process::ProcessStatus::Sleeping => cs.col_status_sleeping,
        crate::system::process::ProcessStatus::DiskSleep => cs.col_status_disk_sleep,
        crate::system::process::ProcessStatus::Stopped => cs.col_status_stopped,
        crate::system::process::ProcessStatus::Zombie => cs.col_status_zombie,
        crate::system::process::ProcessStatus::Unknown => cs.col_status_unknown,
    };

    // Tree prefix
    let tree_prefix = if app.tree_view && proc.depth > 0 {
        let mut prefix = String::new();
        for _ in 0..proc.depth.saturating_sub(1) {
            prefix.push_str("│ ");
        }
        if proc.is_last_child {
            prefix.push_str("└─");
        } else {
            prefix.push_str("├─");
        }
        prefix
    } else {
        String::new()
    };

    // Command column: htop highlights the basename in bold/color
    let cmd_width = width.saturating_sub(fixed_cols_width_visible(app));
    // 'p' toggle: show full command path or just the process name
    let cmd_text = if app.show_full_path {
        proc.command.clone()
    } else {
        proc.name.clone()
    };
    let command_display = format!("{}{}", tree_prefix, cmd_text);
    let command_truncated = truncate_str(&command_display, cmd_width);

    // Highlight process name (basename) within command — htop shows basename in green/bold
    let base_name = &proc.name;

    let base_style = Style::default().bg(bg);

    // Build spans matching htop's exact column order (only visible columns)
    // PID PPID USER PRI NI VIRT RES SHR S CPU% MEM% TIME+ THR IO_R IO_W Command
    let mut spans = Vec::new();
    
    use crate::system::process::ProcessSortField;
    
    if app.visible_columns.contains(&ProcessSortField::Pid) {
        spans.push(Span::styled(format!("{:>6} ", proc.pid), base_style.fg(pid_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::Ppid) {
        spans.push(Span::styled(format!("{:>6} ", proc.ppid), base_style.fg(cs.col_pid)));
    }
    if app.visible_columns.contains(&ProcessSortField::User) {
        spans.push(Span::styled(format!("{:<8} ", truncate_str(&proc.user, 8)), base_style.fg(cs.col_user)));
    }
    if app.visible_columns.contains(&ProcessSortField::Priority) {
        spans.push(Span::styled(format!("{:>3} ", proc.priority), base_style.fg(cs.col_priority)));
    }
    if app.visible_columns.contains(&ProcessSortField::Nice) {
        spans.push(Span::styled(format!("{:>3} ", proc.nice), base_style.fg(default_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::VirtMem) {
        spans.push(Span::styled(format!("{:>6} ", format_bytes(proc.virtual_mem)), base_style.fg(cs.col_priority)));
    }
    if app.visible_columns.contains(&ProcessSortField::ResMem) {
        spans.push(Span::styled(format!("{:>6} ", format_bytes(proc.resident_mem)), base_style.fg(default_fg).add_modifier(Modifier::BOLD)));
    }
    if app.visible_columns.contains(&ProcessSortField::SharedMem) {
        spans.push(Span::styled(format!("{:>6} ", format_bytes(proc.shared_mem)), base_style.fg(default_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::Status) {
        spans.push(Span::styled(format!("{} ", proc.status.symbol()), base_style.fg(status_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::Cpu) {
        spans.push(Span::styled(format!("{:>5.1} ", proc.cpu_usage), base_style.fg(cpu_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::Mem) {
        spans.push(Span::styled(format!("{:>5.1} ", proc.mem_usage), base_style.fg(mem_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::Time) {
        spans.push(Span::styled(format!("{:>9} ", proc.format_time()), base_style.fg(default_fg)));
    }
    if app.visible_columns.contains(&ProcessSortField::Threads) {
        spans.push(Span::styled(format!("{:>3} ", proc.threads), base_style.fg(cs.col_priority)));
    }
    if app.visible_columns.contains(&ProcessSortField::IoReadRate) {
        spans.push(Span::styled(format!("{:>9} ", format_io_rate(proc.io_read_rate)), base_style.fg(Color::Yellow)));
    }
    if app.visible_columns.contains(&ProcessSortField::IoWriteRate) {
        spans.push(Span::styled(format!("{:>9} ", format_io_rate(proc.io_write_rate)), base_style.fg(Color::Magenta)));
    }

    // Command with basename highlighting (htop shows the process name in a different color)
    if let Some(pos) = command_truncated.find(base_name.as_str()) {
        let before = &command_truncated[..pos];
        let name_part = &command_truncated[pos..pos + base_name.len().min(command_truncated.len() - pos)];
        let after = &command_truncated[pos + name_part.len()..];
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), base_style.fg(cs.col_command)));
        }
        spans.push(Span::styled(
            name_part.to_string(),
            base_style.fg(cs.col_command_basename).add_modifier(Modifier::BOLD),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), base_style.fg(cs.col_command)));
        }
    } else {
        spans.push(Span::styled(command_truncated, base_style.fg(cs.col_command)));
    }

    Line::from(spans)
}

/// Build a row for the Net tab view
/// PID USER S CPU% IO_READ IO_WRITE TOTAL_IO MEM% Command
fn build_net_row(
    proc: &crate::system::process::ProcessInfo,
    width: usize,
    app: &App,
    selected: bool,
    tagged: bool,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };
    let _default_fg = if selected { cs.process_selected_fg } else { cs.process_fg };
    let pid_fg = if tagged { Color::Yellow } else { cs.col_pid };
    let base_style = Style::default().bg(bg);

    let status_fg = match &proc.status {
        crate::system::process::ProcessStatus::Running => cs.col_status_running,
        crate::system::process::ProcessStatus::Sleeping => cs.col_status_sleeping,
        _ => cs.col_status_unknown,
    };

    let cpu_fg = if proc.cpu_usage > 90.0 { cs.col_cpu_high }
        else if proc.cpu_usage > 50.0 { cs.col_cpu_medium }
        else { cs.col_cpu_low };

    let mem_fg = if proc.mem_usage > 50.0 { cs.col_mem_high }
        else if proc.mem_usage > 20.0 { cs.col_cpu_medium }
        else { cs.col_mem_normal };

    let read_fg = if proc.io_read_rate > 1_048_576.0 { Color::Red }
        else if proc.io_read_rate > 1024.0 { Color::Yellow }
        else { Color::White };

    let write_fg = if proc.io_write_rate > 1_048_576.0 { Color::Red }
        else if proc.io_write_rate > 1024.0 { Color::Magenta }
        else { Color::White };

    let combined = proc.io_read_rate + proc.io_write_rate;
    let total_fg = if combined > 1_048_576.0 { Color::Red }
        else if combined > 1024.0 { Color::Cyan }
        else { Color::White };

    let cmd_width = width.saturating_sub(net_fixed_cols_width());
    let cmd_text = if app.show_full_path { proc.command.clone() } else { proc.name.clone() };
    let command_truncated = truncate_str(&cmd_text, cmd_width);
    let base_name = &proc.name;

    let mut spans = vec![
        Span::styled(format!("{:>6} ", proc.pid), base_style.fg(pid_fg)),
        Span::styled(format!("{:<8} ", truncate_str(&proc.user, 8)), base_style.fg(cs.col_user)),
        Span::styled(format!("{} ", proc.status.symbol()), base_style.fg(status_fg)),
        Span::styled(format!("{:>5.1} ", proc.cpu_usage), base_style.fg(cpu_fg)),
        Span::styled(format!("{:>9} ", format_io_rate_io_tab(proc.io_read_rate)), base_style.fg(read_fg)),
        Span::styled(format!("{:>9} ", format_io_rate_io_tab(proc.io_write_rate)), base_style.fg(write_fg)),
        Span::styled(format!("{:>9} ", format_io_rate_io_tab(combined)), base_style.fg(total_fg)),
        Span::styled(format!("{:>5.1} ", proc.mem_usage), base_style.fg(mem_fg)),
    ];

    if let Some(pos) = command_truncated.find(base_name.as_str()) {
        let before = &command_truncated[..pos];
        let name_part = &command_truncated[pos..pos + base_name.len().min(command_truncated.len() - pos)];
        let after = &command_truncated[pos + name_part.len()..];
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), base_style.fg(cs.col_command)));
        }
        spans.push(Span::styled(
            name_part.to_string(),
            base_style.fg(cs.col_command_basename).add_modifier(Modifier::BOLD),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), base_style.fg(cs.col_command)));
        }
    } else {
        spans.push(Span::styled(command_truncated, base_style.fg(cs.col_command)));
    }

    Line::from(spans)
}

/// Truncate a string to max characters
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        s.chars().take(max).collect()
    } else {
        s.to_string()
    }
}

/// Format I/O rate (bytes/second) in human-readable form (e.g., "1.5M/s", "23K/s")
fn format_io_rate(rate: f64) -> String {
    if rate == 0.0 {
        "0".to_string()
    } else if rate < 1024.0 {
        format!("{}B/s", rate as u64)
    } else if rate < 1024.0 * 1024.0 {
        format!("{:.1}K/s", rate / 1024.0)
    } else if rate < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1}M/s", rate / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G/s", rate / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format I/O rate for the I/O tab with B/s suffix matching htop
fn format_io_rate_io_tab(rate: f64) -> String {
    if rate == 0.0 {
        "0.00 B/s".to_string()
    } else if rate < 1024.0 {
        format!("{:.2} B/s", rate)
    } else if rate < 1024.0 * 1024.0 {
        format!("{:.2} K/s", rate / 1024.0)
    } else if rate < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} M/s", rate / (1024.0 * 1024.0))
    } else {
        format!("{:.2} G/s", rate / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Map process priority to I/O priority label (htop-style)
/// htop shows "B0"-"B7" for Best Effort class, "R0"-"R7" for Realtime, "id" for Idle
/// We map Windows priority classes:
///   IDLE → id, BELOW_NORMAL → B6, NORMAL → B4, ABOVE_NORMAL → B2, HIGH → B0, REALTIME → R4
fn io_priority_label(priority: i32) -> &'static str {
    match priority {
        4  => "id",   // IDLE_PRIORITY_CLASS
        6  => "B6",   // BELOW_NORMAL
        8  => "B4",   // NORMAL (default)
        10 => "B2",   // ABOVE_NORMAL
        13 => "B0",   // HIGH
        24 => "R4",   // REALTIME
        _  => "B4",   // Default to Normal
    }
}

/// Build a row for the I/O tab view (htop I/O tab columns)
/// PID USER IO DISK_R/Mv DISK_READ DISK_WRITE SWPD% IOD% Command
fn build_io_row(
    proc: &crate::system::process::ProcessInfo,
    width: usize,
    app: &App,
    selected: bool,
    tagged: bool,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };
    let default_fg = if selected { cs.process_selected_fg } else { cs.process_fg };

    let pid_fg = if tagged { Color::Yellow } else { cs.col_pid };
    let base_style = Style::default().bg(bg);

    // I/O rate colors
    let read_fg = if proc.io_read_rate > 1024.0 * 1024.0 {
        Color::Red
    } else if proc.io_read_rate > 1024.0 {
        Color::Yellow
    } else {
        Color::White
    };

    let write_fg = if proc.io_write_rate > 1024.0 * 1024.0 {
        Color::Red
    } else if proc.io_write_rate > 1024.0 {
        Color::Magenta
    } else {
        Color::White
    };

    let combined_rate = proc.io_read_rate + proc.io_write_rate;
    let combined_fg = if combined_rate > 1024.0 * 1024.0 {
        Color::Red
    } else if combined_rate > 1024.0 {
        Color::Cyan
    } else {
        Color::White
    };

    // SWPD%: approximated as 0 on Windows (swap per-process not easily available)
    // We show N/A for most processes, 0.0 otherwise
    let swpd_str = "N/A";
    
    // IOD%: I/O delay percentage (not available on Windows, show N/A)
    let iod_str = "N/A";

    // I/O priority label
    let io_prio = io_priority_label(proc.priority);

    // Command column width
    let cmd_width = width.saturating_sub(io_fixed_cols_width());
    let cmd_text = if app.show_full_path {
        proc.command.clone()
    } else {
        proc.name.clone()
    };

    // Tree prefix
    let tree_prefix = if app.tree_view && proc.depth > 0 {
        let mut prefix = String::new();
        for _ in 0..proc.depth.saturating_sub(1) {
            prefix.push_str("│ ");
        }
        if proc.is_last_child {
            prefix.push_str("└─");
        } else {
            prefix.push_str("├─");
        }
        prefix
    } else {
        String::new()
    };

    let command_display = format!("{}{}", tree_prefix, cmd_text);
    let command_truncated = truncate_str(&command_display, cmd_width);
    let base_name = &proc.name;

    let mut spans = vec![
        Span::styled(format!("{:>6} ", proc.pid), base_style.fg(pid_fg)),
        Span::styled(format!("{:<8} ", truncate_str(&proc.user, 8)), base_style.fg(cs.col_user)),
        Span::styled(format!("{:<3} ", io_prio), base_style.fg(default_fg)),
        Span::styled(format!("{:>9} ", format_io_rate_io_tab(combined_rate)), base_style.fg(combined_fg)),
        Span::styled(format!("{:>9} ", format_io_rate_io_tab(proc.io_read_rate)), base_style.fg(read_fg)),
        Span::styled(format!("{:>10} ", format_io_rate_io_tab(proc.io_write_rate)), base_style.fg(write_fg)),
        Span::styled(format!("{:>5} ", swpd_str), base_style.fg(cs.col_status_unknown)),
        Span::styled(format!("{:>5} ", iod_str), base_style.fg(cs.col_status_unknown)),
    ];

    // Command with basename highlighting
    if let Some(pos) = command_truncated.find(base_name.as_str()) {
        let before = &command_truncated[..pos];
        let name_part = &command_truncated[pos..pos + base_name.len().min(command_truncated.len() - pos)];
        let after = &command_truncated[pos + name_part.len()..];
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), base_style.fg(cs.col_command)));
        }
        spans.push(Span::styled(
            name_part.to_string(),
            base_style.fg(cs.col_command_basename).add_modifier(Modifier::BOLD),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), base_style.fg(cs.col_command)));
        }
    } else {
        spans.push(Span::styled(command_truncated, base_style.fg(cs.col_command)));
    }

    Line::from(spans)
}
