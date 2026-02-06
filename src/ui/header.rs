use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::system::memory::format_bytes;

/// Draw the complete header area in htop's exact layout:
///
/// LEFT COLUMN (50%):            RIGHT COLUMN (50%):
///   0 [||||     25.3%]            8 [||||||     42.1%]
///   1 [||||||   43.2%]            9 [||||       30.0%]
///   ...                           ...
///   Mem[||||used|||cache|    5.2G/16.0G]
///   Swp[||               0.8G/8.0G]
///
/// RIGHT side overlays (after CPUs): Tasks, Load average, Uptime
pub fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let cores = &app.cpu_info.cores;
    let half = (cores.len() + 1) / 2; // Left column gets ceil(n/2) cores

    // Split into left and right columns (htop classic layout)
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_col = columns[0];
    let right_col = columns[1];

    // Left column: first half of CPU cores + Mem + Swp
    let left_rows = half + 2; // cpu cores + mem + swap
    let left_constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(1))
        .take(left_rows)
        .collect();
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(left_constraints)
        .split(left_col);

    // Right column: second half of CPU cores + Tasks + Load + Uptime
    let right_cpu_count = cores.len() - half;
    let right_rows = right_cpu_count + 3; // cpu cores + tasks + load avg + uptime
    let right_constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(1))
        .take(right_rows)
        .collect();
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(right_constraints)
        .split(right_col);

    // --- Left column: CPU cores 0..half ---
    for i in 0..half {
        if i < cores.len() {
            draw_cpu_bar(f, &cores[i], left_chunks[i]);
        }
    }

    // --- Left column: Mem bar ---
    draw_memory_bar(f, app, left_chunks[half]);

    // --- Left column: Swap bar ---
    draw_swap_bar(f, app, left_chunks[half + 1]);

    // --- Right column: CPU cores half..end ---
    for i in 0..right_cpu_count {
        let core_idx = half + i;
        if core_idx < cores.len() {
            draw_cpu_bar(f, &cores[core_idx], right_chunks[i]);
        }
    }

    // --- Right column: Tasks line ---
    if right_cpu_count < right_chunks.len() {
        draw_tasks_line(f, app, right_chunks[right_cpu_count]);
    }

    // --- Right column: Load average line ---
    if right_cpu_count + 1 < right_chunks.len() {
        draw_load_line(f, app, right_chunks[right_cpu_count + 1]);
    }

    // --- Right column: Uptime line ---
    if right_cpu_count + 2 < right_chunks.len() {
        draw_uptime_line(f, app, right_chunks[right_cpu_count + 2]);
    }
}

/// Draw a single CPU core usage bar with htop's multi-color scheme:
///   Green  = normal (user) processes
///   Red    = kernel / system processes
///   Blue   = low priority (nice > 0)
///   Cyan   = steal / virtualization overhead
///
/// On Windows we can't separate user/kernel, so we approximate:
///   Green portion  = 0-60% of usage (user estimate)
///   Red portion    = 60-100% of usage (kernel estimate)
///   This gives the visual multi-color effect matching htop.
fn draw_cpu_bar(f: &mut Frame, core: &crate::system::cpu::CpuCore, area: Rect) {
    let usage = core.usage_percent;
    let label = format!("{:>2}", core.id);
    let pct_label = format!("{:>5.1}%", usage);

    let bar_width = area.width as usize;
    let prefix_len = label.len() + 1; // "NN "
    let suffix_len = pct_label.len() + 1;
    let bracket_len = 2; // [ ]
    let available = bar_width.saturating_sub(prefix_len + suffix_len + bracket_len);

    let total_filled = ((usage as f64 / 100.0) * available as f64) as usize;
    let total_filled = total_filled.min(available);

    // Split into green (user ~70%) and red (kernel ~30%) portions
    let green_portion = (total_filled as f64 * 0.7) as usize;
    let red_portion = total_filled.saturating_sub(green_portion);
    let empty = available.saturating_sub(total_filled);

    let green_bar: String = "|".repeat(green_portion);
    let red_bar: String = "|".repeat(red_portion);
    let empty_str: String = " ".repeat(empty);

    let line = Line::from(vec![
        Span::styled(
            format!("{} ", label),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(Color::White)),
        Span::styled(green_bar, Style::default().fg(Color::Green)),
        Span::styled(red_bar, Style::default().fg(Color::Red)),
        Span::styled(empty_str, Style::default().fg(Color::DarkGray)),
        Span::styled("]", Style::default().fg(Color::White)),
        Span::styled(pct_label, Style::default().fg(Color::White)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw the memory usage bar with htop's multi-color scheme:
///   Green  = used memory pages
///   Blue   = buffer pages
///   Yellow = cache pages
fn draw_memory_bar(f: &mut Frame, app: &App, area: Rect) {
    let mem = &app.memory_info;
    let total = mem.total_mem as f64;
    if total == 0.0 {
        return;
    }

    let used_frac = mem.used_mem as f64 / total;
    let buffer_frac = mem.buffered_mem as f64 / total;
    let cache_frac = mem.cached_mem as f64 / total;

    let used_str = format_bytes(mem.used_mem);
    let total_str = format_bytes(mem.total_mem);
    let suffix = format!("{}/{}", used_str, total_str);

    let prefix = "Mem";
    let bar_width = area.width as usize;
    let bracket_len = 2;
    let available = bar_width.saturating_sub(prefix.len() + suffix.len() + bracket_len + 1);

    let green_len = ((used_frac) * available as f64) as usize;
    let blue_len = ((buffer_frac) * available as f64) as usize;
    let yellow_len = ((cache_frac) * available as f64) as usize;
    let total_filled = (green_len + blue_len + yellow_len).min(available);
    let empty = available.saturating_sub(total_filled);

    let line = Line::from(vec![
        Span::styled(
            prefix,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(Color::White)),
        Span::styled("|".repeat(green_len), Style::default().fg(Color::Green)),
        Span::styled("|".repeat(blue_len), Style::default().fg(Color::Blue)),
        Span::styled("|".repeat(yellow_len), Style::default().fg(Color::Yellow)),
        Span::styled(" ".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::styled("]", Style::default().fg(Color::White)),
        Span::styled(suffix, Style::default().fg(Color::White)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw the swap usage bar (green only, like htop)
fn draw_swap_bar(f: &mut Frame, app: &App, area: Rect) {
    let mem = &app.memory_info;
    let total = mem.total_swap as f64;
    let usage_frac = if total > 0.0 { mem.used_swap as f64 / total } else { 0.0 };

    let used_str = format_bytes(mem.used_swap);
    let total_str = format_bytes(mem.total_swap);
    let suffix = format!("{}/{}", used_str, total_str);

    let prefix = "Swp";
    let bar_width = area.width as usize;
    let bracket_len = 2;
    let available = bar_width.saturating_sub(prefix.len() + suffix.len() + bracket_len + 1);

    let filled = ((usage_frac) * available as f64) as usize;
    let filled = filled.min(available);
    let empty = available.saturating_sub(filled);

    let color = if usage_frac < 0.5 {
        Color::Green
    } else if usage_frac < 0.8 {
        Color::Yellow
    } else {
        Color::Red
    };

    let line = Line::from(vec![
        Span::styled(
            prefix,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(Color::White)),
        Span::styled("|".repeat(filled), Style::default().fg(color)),
        Span::styled(" ".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::styled("]", Style::default().fg(Color::White)),
        Span::styled(suffix, Style::default().fg(Color::White)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw: "Tasks: 312, 1024 thr; 5 running"
fn draw_tasks_line(f: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled("Tasks: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}", app.total_tasks), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!(", "), Style::default().fg(Color::White)),
        Span::styled(format!("{}", app.total_threads), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(" thr; ", Style::default().fg(Color::White)),
        Span::styled(format!("{}", app.running_tasks), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(" running", Style::default().fg(Color::White)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Draw: "Load average: 0.28 0.45 0.47"
fn draw_load_line(f: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            "Load average: ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:.2} ", app.load_avg_1),
            Style::default().fg(if app.load_avg_1 > app.cpu_info.cores.len() as f64 { Color::Red } else { Color::White }).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:.2} ", app.load_avg_5),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:.2}", app.load_avg_15),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Draw: "Uptime: 05:12:01"
fn draw_uptime_line(f: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            "Uptime: ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format_uptime(app.uptime_seconds), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Format uptime as DD days, HH:MM:SS (matching htop)
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{} days, {:02}:{:02}:{:02}", days, hours, minutes, secs)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }
}
