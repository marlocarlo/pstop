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
/// 4th element = display priority (higher = more important, hidden last on narrow terminals)
pub const HEADERS: &[(&str, u16, ProcessSortField, u8)] = &[
    ("PID",        7,  ProcessSortField::Pid,         90),
    ("PPID",       7,  ProcessSortField::Ppid,        15),
    ("USER",       9,  ProcessSortField::User,        80),
    ("PRI",        4,  ProcessSortField::Priority,    20),
    ("NI",         4,  ProcessSortField::Nice,        15),
    ("VIRT",       7,  ProcessSortField::VirtMem,     30),
    ("RES",        7,  ProcessSortField::ResMem,      55),
    ("SHR",        7,  ProcessSortField::SharedMem,   25),
    ("S",          2,  ProcessSortField::Status,      45),
    ("CPU%",       6,  ProcessSortField::Cpu,         95),
    ("MEM%",       6,  ProcessSortField::Mem,         85),
    ("TIME+",     10,  ProcessSortField::Time,        50),
    ("THR",        4,  ProcessSortField::Threads,     25),
    ("IO_R",      10,  ProcessSortField::IoReadRate,  10),
    ("IO_W",      10,  ProcessSortField::IoWriteRate,  8),
    ("Command",    0,  ProcessSortField::Command,    100), // 0 = takes remaining space
];

/// htop I/O tab column headers
/// PID USER IO DISK R/W DISK READ DISK WRITE SWPD% IOD% Command
pub const IO_HEADERS: &[(&str, u16, ProcessSortField, u8)] = &[
    ("PID",         7,  ProcessSortField::Pid,          90),
    ("USER",        9,  ProcessSortField::User,         80),
    ("IO",          4,  ProcessSortField::Priority,     50),
    ("DISK R/Mv",  10,  ProcessSortField::IoRate,       85),
    ("DISK READ",  10,  ProcessSortField::IoReadRate,   70),
    ("DISK WRITE", 11,  ProcessSortField::IoWriteRate,  65),
    ("SWPD%",       6,  ProcessSortField::Mem,          20),
    ("IOD%",        6,  ProcessSortField::Cpu,          15),
    ("Command",     0,  ProcessSortField::Command,     100),
];

/// Network bandwidth tab column headers (Net tab - per-process bandwidth)
/// Shows live download/upload rates aggregated per process.
pub const NET_HEADERS: &[(&str, u16, ProcessSortField, u8)] = &[
    ("PID",          7,  ProcessSortField::Pid,         90),
    ("Process",     15,  ProcessSortField::Command,    100),
    ("Download",    12,  ProcessSortField::IoReadRate,   95),
    ("Upload",      12,  ProcessSortField::IoWriteRate,  85),
    ("Connections",  0,  ProcessSortField::Nice,         70),
];

/// GPU tab column headers (per-process GPU usage)
pub const GPU_HEADERS: &[(&str, u16, ProcessSortField, u8)] = &[
    ("PID",        7,  ProcessSortField::Pid,         90),
    ("Process",   15,  ProcessSortField::Command,    100),
    ("GPU%",       7,  ProcessSortField::Cpu,         95),
    ("Engine",    14,  ProcessSortField::Status,      80),
    ("Ded.Mem",   10,  ProcessSortField::ResMem,      85),
    ("Shr.Mem",   10,  ProcessSortField::SharedMem,   70),
    ("Total",      0,  ProcessSortField::VirtMem,     60),
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
        ProcessTab::Gpu => GPU_HEADERS,
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

    // Compute which columns to display (user-visible ∩ auto-hide by width priority)
    let base_visible: std::collections::HashSet<ProcessSortField> = match app.active_tab {
        ProcessTab::Main => app.visible_columns.clone(),
        _ => headers.iter().map(|(_, _, f, _)| *f).collect(),
    };
    let active_sort = app.active_sort_field();
    let active_ascending = app.active_sort_ascending();
    let display_cols = compute_display_columns(headers, &base_visible, area.width, active_sort);

    // Build header spans with sort indicator
    let mut header_spans: Vec<Span> = Vec::new();
    for (name, width, sort_field, _prio) in headers {
        // Skip columns not in the computed display set
        if !display_cols.contains(sort_field) {
            continue;
        }
        
        let is_sorted = *sort_field == active_sort;
        let fixed_w = fixed_cols_width_for(headers, &display_cols);
        let w = if *width == 0 { (area.width as usize).saturating_sub(fixed_w) } else { *width as usize };

        let display = if is_sorted {
            let arrow = if active_ascending { "▲" } else { "▼" };
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

    // ── Render rows based on active tab ──
    match app.active_tab {
        ProcessTab::Main | ProcessTab::Io => {
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
                    ProcessTab::Main => build_process_row(proc, row_area.width as usize, app, is_selected, is_tagged, &display_cols),
                    ProcessTab::Io => build_io_row(proc, row_area.width as usize, app, is_selected, is_tagged, &display_cols),
                    _ => unreachable!(),
                };
                f.render_widget(Paragraph::new(row_line), row_area);
            }
        }

        ProcessTab::Net => {
            let start = app.net_scroll_offset;
            let end = (start + visible).min(app.net_processes.len());

            for (i, row_idx) in (start..end).enumerate() {
                let proc_net = &app.net_processes[row_idx];
                let is_selected = row_idx == app.net_selected_index;

                let row_area = Rect {
                    x: proc_area.x,
                    y: proc_area.y + i as u16,
                    width: proc_area.width,
                    height: 1,
                };

                let row_line = build_net_bandwidth_row(proc_net, row_area.width as usize, app, is_selected);
                f.render_widget(Paragraph::new(row_line), row_area);
            }

            if app.net_processes.is_empty() {
                let msg_area = Rect {
                    x: proc_area.x,
                    y: proc_area.y,
                    width: proc_area.width,
                    height: 1,
                };
                let msg = Line::from(Span::styled(
                    "  No processes with active network connections found",
                    Style::default().fg(Color::DarkGray),
                ));
                f.render_widget(Paragraph::new(msg), msg_area);
            }
        }

        ProcessTab::Gpu => {
            let start = app.gpu_scroll_offset;
            let end = (start + visible).min(app.gpu_processes.len());

            for (i, row_idx) in (start..end).enumerate() {
                let gpu_proc = &app.gpu_processes[row_idx];
                let is_selected = row_idx == app.gpu_selected_index;

                let row_area = Rect {
                    x: proc_area.x,
                    y: proc_area.y + i as u16,
                    width: proc_area.width,
                    height: 1,
                };

                let row_line = build_gpu_row(gpu_proc, row_area.width as usize, app, is_selected);
                f.render_widget(Paragraph::new(row_line), row_area);
            }

            // If no GPU data, show a message
            if app.gpu_processes.is_empty() {
                let msg_area = Rect {
                    x: proc_area.x,
                    y: proc_area.y,
                    width: proc_area.width,
                    height: 1,
                };
                let msg = Line::from(Span::styled(
                    "  No GPU data available (requires Windows 10 1709+ with WDDM 2.0+ GPU)",
                    Style::default().fg(Color::DarkGray),
                ));
                f.render_widget(Paragraph::new(msg), msg_area);
            }
        }
    }

    // Search / Filter bar
    if let Some(bar_rect) = bar_area {
        let bar_line = if app.mode == AppMode::Search {
            let mut spans = vec![
                Span::styled("Search: ", Style::default().fg(cs.search_label).add_modifier(Modifier::BOLD)),
                Span::styled(app.search_query.clone(), Style::default().fg(cs.search_text)),
                Span::styled("_", Style::default().fg(cs.search_text).add_modifier(Modifier::SLOW_BLINK)),
            ];
            if app.search_not_found {
                spans.push(Span::styled("  Not found", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
            }
            Line::from(spans)
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

/// Minimum width reserved for the Command column before auto-hiding other columns.
/// When the terminal is too narrow, low-priority columns are hidden progressively
/// (like htop) to ensure Command, CPU%, MEM%, USER, PID remain visible.
const MIN_COMMAND_WIDTH: usize = 20;

/// Compute which columns should actually be displayed given the terminal width.
/// Progressively hides low-priority columns (lowest priority first) until
/// the Command column has at least MIN_COMMAND_WIDTH characters.
/// The currently sorted column is never auto-hidden.
pub fn compute_display_columns(
    headers: &[(&str, u16, ProcessSortField, u8)],
    visible: &std::collections::HashSet<ProcessSortField>,
    width: u16,
    sort_field: ProcessSortField,
) -> std::collections::HashSet<ProcessSortField> {
    // Collect removable fixed-width columns sorted by priority (lowest first)
    let mut removable: Vec<(ProcessSortField, u16, u8)> = headers.iter()
        .filter(|(_, w, field, _)| *w > 0 && visible.contains(field))
        .map(|(_, w, field, prio)| (*field, *w, *prio))
        .collect();
    removable.sort_by_key(|(_, _, prio)| *prio);

    let mut result = visible.clone();

    loop {
        let fixed_w: usize = headers.iter()
            .filter(|(_, w, field, _)| *w > 0 && result.contains(field))
            .map(|(_, w, _, _)| *w as usize + 1)
            .sum();
        let cmd_space = (width as usize).saturating_sub(fixed_w);

        if cmd_space >= MIN_COMMAND_WIDTH {
            break;
        }

        // Find the lowest-priority column still in result (skip sort field & Command)
        if let Some(pos) = removable.iter().position(|(field, _, _)| {
            result.contains(field)
                && *field != sort_field
                && *field != ProcessSortField::Command
        }) {
            result.remove(&removable[pos].0);
        } else {
            break; // nothing left to remove
        }
    }

    result
}

/// Total width of fixed-width columns in the given display set
fn fixed_cols_width_for(
    headers: &[(&str, u16, ProcessSortField, u8)],
    display_cols: &std::collections::HashSet<ProcessSortField>,
) -> usize {
    headers.iter()
        .filter(|(_, _, field, _)| display_cols.contains(field))
        .map(|(_, w, _, _)| if *w > 0 { *w as usize + 1 } else { 0 })
        .sum()
}

/// Build a single process row as a styled Line (matching htop's exact columns)
fn build_process_row(
    proc: &crate::system::process::ProcessInfo,
    width: usize,
    app: &App,
    selected: bool,
    tagged: bool,
    display_cols: &std::collections::HashSet<ProcessSortField>,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };

    // shadow_other_users: dim processes owned by other users
    let is_other_user = app.shadow_other_users
        && !selected
        && proc.user.to_lowercase() != app.current_user;
    let default_fg = if is_other_user {
        cs.process_shadow
    } else if selected {
        cs.process_selected_fg
    } else {
        cs.process_fg
    };

    let pid_fg = if tagged { Color::Yellow } else if is_other_user { cs.process_shadow } else { cs.col_pid };

    let cpu_fg = if is_other_user { cs.process_shadow }
        else if proc.cpu_usage > 90.0 { cs.col_cpu_high }
        else if proc.cpu_usage > 50.0 { cs.col_cpu_medium }
        else { cs.col_cpu_low };

    let mem_fg = if is_other_user { cs.process_shadow }
        else if proc.mem_usage > 50.0 { cs.col_mem_high }
        else if proc.mem_usage > 20.0 { cs.col_cpu_medium }
        else { cs.col_mem_normal };

    let status_fg = if is_other_user { cs.process_shadow } else { match &proc.status {
        crate::system::process::ProcessStatus::Running => cs.col_status_running,
        crate::system::process::ProcessStatus::Sleeping => cs.col_status_sleeping,
        crate::system::process::ProcessStatus::DiskSleep => cs.col_status_disk_sleep,
        crate::system::process::ProcessStatus::Stopped => cs.col_status_stopped,
        crate::system::process::ProcessStatus::Zombie => cs.col_status_zombie,
        crate::system::process::ProcessStatus::Unknown => cs.col_status_unknown,
    }};

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

    // Command column: show_merged_command merges name + full command
    let cmd_width = width.saturating_sub(fixed_cols_width_for(HEADERS, display_cols));
    let cmd_text = if app.show_merged_command {
        // Merged: "name command_args" (like htop's merged command)
        if proc.command != proc.name && !proc.command.is_empty() {
            format!("{} {}", proc.name, proc.command)
        } else {
            proc.name.clone()
        }
    } else if app.show_full_path {
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
    
    if display_cols.contains(&ProcessSortField::Pid) {
        spans.push(Span::styled(format!("{:>6} ", proc.pid), base_style.fg(pid_fg)));
    }
    if display_cols.contains(&ProcessSortField::Ppid) {
        spans.push(Span::styled(format!("{:>6} ", proc.ppid), base_style.fg(if is_other_user { cs.process_shadow } else { cs.col_pid })));
    }
    if display_cols.contains(&ProcessSortField::User) {
        spans.push(Span::styled(format!("{:<8} ", truncate_str(&proc.user, 8)), base_style.fg(if is_other_user { cs.process_shadow } else { cs.col_user })));
    }
    if display_cols.contains(&ProcessSortField::Priority) {
        spans.push(Span::styled(format!("{:>3} ", proc.priority), base_style.fg(if is_other_user { cs.process_shadow } else { cs.col_priority })));
    }
    if display_cols.contains(&ProcessSortField::Nice) {
        spans.push(Span::styled(format!("{:>3} ", proc.nice), base_style.fg(default_fg)));
    }
    if display_cols.contains(&ProcessSortField::VirtMem) {
        // highlight_megabytes: color large memory values
        let virt_fg = if is_other_user { cs.process_shadow }
            else if app.highlight_megabytes && proc.virtual_mem >= 1024 * 1024 * 1024 { cs.col_mem_high }
            else if app.highlight_megabytes && proc.virtual_mem >= 1024 * 1024 { cs.col_priority }
            else { default_fg };
        spans.push(Span::styled(format!("{:>6} ", format_bytes(proc.virtual_mem)), base_style.fg(virt_fg)));
    }
    if display_cols.contains(&ProcessSortField::ResMem) {
        let res_fg = if is_other_user { cs.process_shadow }
            else if app.highlight_megabytes && proc.resident_mem >= 1024 * 1024 * 1024 { cs.col_mem_high }
            else if app.highlight_megabytes && proc.resident_mem >= 1024 * 1024 { Color::Yellow }
            else { default_fg };
        spans.push(Span::styled(format!("{:>6} ", format_bytes(proc.resident_mem)), base_style.fg(res_fg).add_modifier(Modifier::BOLD)));
    }
    if display_cols.contains(&ProcessSortField::SharedMem) {
        spans.push(Span::styled(format!("{:>6} ", format_bytes(proc.shared_mem)), base_style.fg(default_fg)));
    }
    if display_cols.contains(&ProcessSortField::Status) {
        spans.push(Span::styled(format!("{} ", proc.status.symbol()), base_style.fg(status_fg)));
    }
    if display_cols.contains(&ProcessSortField::Cpu) {
        spans.push(Span::styled(format!("{:>5.1} ", proc.cpu_usage), base_style.fg(cpu_fg)));
    }
    if display_cols.contains(&ProcessSortField::Mem) {
        spans.push(Span::styled(format!("{:>5.1} ", proc.mem_usage), base_style.fg(mem_fg)));
    }
    if display_cols.contains(&ProcessSortField::Time) {
        spans.push(Span::styled(format!("{:>9} ", proc.format_time()), base_style.fg(default_fg)));
    }
    if display_cols.contains(&ProcessSortField::Threads) {
        // highlight_threads: color thread count differently
        let thr_fg = if is_other_user { cs.process_shadow }
            else if app.highlight_threads && proc.threads > 10 { cs.col_thread }
            else if app.highlight_threads { cs.col_priority }
            else { cs.col_priority };
        spans.push(Span::styled(format!("{:>3} ", proc.threads), base_style.fg(thr_fg)));
    }
    if display_cols.contains(&ProcessSortField::IoReadRate) {
        spans.push(Span::styled(format!("{:>9} ", format_io_rate(proc.io_read_rate)), base_style.fg(if is_other_user { cs.process_shadow } else { Color::Yellow })));
    }
    if display_cols.contains(&ProcessSortField::IoWriteRate) {
        spans.push(Span::styled(format!("{:>9} ", format_io_rate(proc.io_write_rate)), base_style.fg(if is_other_user { cs.process_shadow } else { Color::Magenta })));
    }

    // Command with basename highlighting (htop shows the process name in a different color)
    // Controlled by highlight_base_name display option
    let cmd_fg = if is_other_user { cs.process_shadow } else { cs.col_command };
    let cmd_base_fg = if is_other_user { cs.process_shadow } else { cs.col_command_basename };
    if app.highlight_base_name {
        if let Some(pos) = command_truncated.find(base_name.as_str()) {
            let before = &command_truncated[..pos];
            let name_part = &command_truncated[pos..pos + base_name.len().min(command_truncated.len() - pos)];
            let after = &command_truncated[pos + name_part.len()..];
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), base_style.fg(cmd_fg)));
            }
            spans.push(Span::styled(
                name_part.to_string(),
                base_style.fg(cmd_base_fg).add_modifier(Modifier::BOLD),
            ));
            if !after.is_empty() {
                spans.push(Span::styled(after.to_string(), base_style.fg(cmd_fg)));
            }
        } else {
            spans.push(Span::styled(command_truncated, base_style.fg(cmd_fg)));
        }
    } else {
        spans.push(Span::styled(command_truncated, base_style.fg(cmd_fg)));
    }

    Line::from(spans)
}

/// Build a row for the Net tab (per-process bandwidth)
/// PID  Process  Download  Upload  Connections
fn build_net_bandwidth_row(
    proc_net: &crate::system::netstat::ProcessNetBandwidth,
    width: usize,
    app: &App,
    selected: bool,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };
    let base_style = Style::default().bg(bg);
    let default_fg = if selected { cs.process_selected_fg } else { cs.process_fg };

    let dl_str = format_bandwidth(proc_net.recv_bytes_per_sec);
    let ul_str = format_bandwidth(proc_net.send_bytes_per_sec);

    let dl_color = bandwidth_color(proc_net.recv_bytes_per_sec);
    let ul_color = bandwidth_color(proc_net.send_bytes_per_sec);

    // Fixed: PID(7) + Process(15) + Download(12) + Upload(12) = 46
    let conn_width = width.saturating_sub(46);

    let mut spans = Vec::new();
    spans.push(Span::styled(format!("{:>6} ", proc_net.pid), base_style.fg(cs.col_pid)));
    spans.push(Span::styled(
        format!("{:<14} ", truncate_str(&proc_net.name, 14)),
        base_style.fg(cs.col_command_basename).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(format!("{:>11} ", dl_str), base_style.fg(dl_color)));
    spans.push(Span::styled(format!("{:>11} ", ul_str), base_style.fg(ul_color)));
    spans.push(Span::styled(
        format!("{:<width$}", proc_net.connection_count, width = conn_width),
        base_style.fg(default_fg),
    ));

    Line::from(spans)
}

/// Format bytes/sec as human-readable bandwidth with auto-scaling units
fn format_bandwidth(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_073_741_824.0 {
        format!("{:.1} GB/s", bytes_per_sec / 1_073_741_824.0)
    } else if bytes_per_sec >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1_048_576.0)
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else if bytes_per_sec >= 1.0 {
        format!("{:.0} B/s", bytes_per_sec)
    } else {
        "0 B/s".to_string()
    }
}

/// Color code bandwidth values: gray(idle) → green(low) → yellow(medium) → red(high)
fn bandwidth_color(bytes_per_sec: f64) -> Color {
    if bytes_per_sec >= 10_485_760.0 {      // > 10 MB/s
        Color::Red
    } else if bytes_per_sec >= 1_048_576.0 { // > 1 MB/s
        Color::Yellow
    } else if bytes_per_sec >= 1024.0 {      // > 1 KB/s
        Color::Green
    } else {
        Color::DarkGray
    }
}

/// Build a row for the GPU tab (per-process GPU usage)
/// PID  Process  GPU%  Engine  Ded.Mem  Shr.Mem  Total
fn build_gpu_row(
    gpu_proc: &crate::system::gpu::GpuProcessInfo,
    width: usize,
    app: &App,
    selected: bool,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };
    let base_style = Style::default().bg(bg);

    // GPU usage color
    let gpu_fg = if gpu_proc.gpu_usage > 80.0 { cs.col_cpu_high }
        else if gpu_proc.gpu_usage > 30.0 { cs.col_cpu_medium }
        else if gpu_proc.gpu_usage > 0.1 { Color::Green }
        else { Color::DarkGray };

    // Memory color
    let ded_fg = if gpu_proc.dedicated_mem > 512 * 1024 * 1024 { Color::Red }
        else if gpu_proc.dedicated_mem > 64 * 1024 * 1024 { Color::Yellow }
        else { Color::White };

    let shr_fg = if gpu_proc.shared_mem > 256 * 1024 * 1024 { Color::Magenta }
        else if gpu_proc.shared_mem > 32 * 1024 * 1024 { Color::Cyan }
        else { Color::White };

    let total_mem = gpu_proc.dedicated_mem + gpu_proc.shared_mem;

    // Look up process name from the process list
    let proc_name = app.processes.iter()
        .find(|p| p.pid == gpu_proc.pid)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| format!("PID {}", gpu_proc.pid));

    let engine_str = if gpu_proc.engine_type.is_empty() { "---" } else { &gpu_proc.engine_type };

    // Fixed columns: PID(7) + Process(15) + GPU%(7) + Engine(14) + Ded.Mem(10) + Shr.Mem(10) = 63
    let total_width = width.saturating_sub(63);

    let mut spans = Vec::new();
    spans.push(Span::styled(format!("{:>6} ", gpu_proc.pid), base_style.fg(cs.col_pid)));
    spans.push(Span::styled(format!("{:<14} ", truncate_str(&proc_name, 14)), base_style.fg(cs.col_command_basename).add_modifier(Modifier::BOLD)));
    spans.push(Span::styled(format!("{:>5.1}% ", gpu_proc.gpu_usage), base_style.fg(gpu_fg)));
    spans.push(Span::styled(format!("{:<13} ", truncate_str(engine_str, 13)), base_style.fg(Color::Cyan)));
    spans.push(Span::styled(format!("{:>9} ", format_bytes(gpu_proc.dedicated_mem)), base_style.fg(ded_fg)));
    spans.push(Span::styled(format!("{:>9} ", format_bytes(gpu_proc.shared_mem)), base_style.fg(shr_fg)));
    spans.push(Span::styled(format!("{:<width$}", format_bytes(total_mem), width = total_width), base_style.fg(Color::White)));

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
        "0B/s".to_string()
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
    display_cols: &std::collections::HashSet<ProcessSortField>,
) -> Line<'static> {
    let cs = &app.color_scheme;
    let bg = if selected { cs.process_selected_bg } else { cs.process_bg };

    let is_other_user = app.shadow_other_users
        && !selected
        && proc.user.to_lowercase() != app.current_user;
    let default_fg = if is_other_user { cs.process_shadow }
        else if selected { cs.process_selected_fg }
        else { cs.process_fg };

    let pid_fg = if tagged { Color::Yellow } else if is_other_user { cs.process_shadow } else { cs.col_pid };
    let base_style = Style::default().bg(bg);

    // I/O rate colors
    let read_fg = if is_other_user { cs.process_shadow }
        else if proc.io_read_rate > 1024.0 * 1024.0 { Color::Red }
        else if proc.io_read_rate > 1024.0 { Color::Yellow }
        else { Color::White };

    let write_fg = if is_other_user { cs.process_shadow }
        else if proc.io_write_rate > 1024.0 * 1024.0 { Color::Red }
        else if proc.io_write_rate > 1024.0 { Color::Magenta }
        else { Color::White };

    let combined_rate = proc.io_read_rate + proc.io_write_rate;
    let combined_fg = if is_other_user { cs.process_shadow }
        else if combined_rate > 1024.0 * 1024.0 { Color::Red }
        else if combined_rate > 1024.0 { Color::Cyan }
        else { Color::White };

    // SWPD%: approximated as 0 on Windows (swap per-process not easily available)
    // We show N/A for most processes, 0.0 otherwise
    let swpd_str = "N/A";
    
    // IOD%: I/O delay percentage (not available on Windows, show N/A)
    let iod_str = "N/A";

    // I/O priority label
    let io_prio = io_priority_label(proc.priority);

    // Command column width
    let cmd_width = width.saturating_sub(fixed_cols_width_for(IO_HEADERS, display_cols));
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

    let mut spans = Vec::new();
    if display_cols.contains(&ProcessSortField::Pid) {
        spans.push(Span::styled(format!("{:>6} ", proc.pid), base_style.fg(pid_fg)));
    }
    if display_cols.contains(&ProcessSortField::User) {
        spans.push(Span::styled(format!("{:<8} ", truncate_str(&proc.user, 8)), base_style.fg(if is_other_user { cs.process_shadow } else { cs.col_user })));
    }
    if display_cols.contains(&ProcessSortField::Priority) {
        spans.push(Span::styled(format!("{:<3} ", io_prio), base_style.fg(default_fg)));
    }
    if display_cols.contains(&ProcessSortField::IoRate) {
        spans.push(Span::styled(format!("{:>9} ", format_io_rate_io_tab(combined_rate)), base_style.fg(combined_fg)));
    }
    if display_cols.contains(&ProcessSortField::IoReadRate) {
        spans.push(Span::styled(format!("{:>9} ", format_io_rate_io_tab(proc.io_read_rate)), base_style.fg(read_fg)));
    }
    if display_cols.contains(&ProcessSortField::IoWriteRate) {
        spans.push(Span::styled(format!("{:>10} ", format_io_rate_io_tab(proc.io_write_rate)), base_style.fg(write_fg)));
    }
    if display_cols.contains(&ProcessSortField::Mem) {
        spans.push(Span::styled(format!("{:>5} ", swpd_str), base_style.fg(if is_other_user { cs.process_shadow } else { cs.col_status_unknown })));
    }
    if display_cols.contains(&ProcessSortField::Cpu) {
        spans.push(Span::styled(format!("{:>5} ", iod_str), base_style.fg(if is_other_user { cs.process_shadow } else { cs.col_status_unknown })));
    }

    // Command with basename highlighting
    let cmd_fg = if is_other_user { cs.process_shadow } else { cs.col_command };
    let cmd_base_fg = if is_other_user { cs.process_shadow } else { cs.col_command_basename };
    if let Some(pos) = command_truncated.find(base_name.as_str()) {
        let before = &command_truncated[..pos];
        let name_part = &command_truncated[pos..pos + base_name.len().min(command_truncated.len() - pos)];
        let after = &command_truncated[pos + name_part.len()..];
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), base_style.fg(cmd_fg)));
        }
        spans.push(Span::styled(
            name_part.to_string(),
            base_style.fg(cmd_base_fg).add_modifier(Modifier::BOLD),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), base_style.fg(cmd_fg)));
        }
    } else {
        spans.push(Span::styled(command_truncated, base_style.fg(cmd_fg)));
    }

    Line::from(spans)
}
