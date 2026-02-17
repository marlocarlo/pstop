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
    // Compact mode: single aggregate CPU bar + memory bar
    if app.compact_mode {
        draw_compact_header(f, app, area);
        return;
    }

    let cores = &app.cpu_info.cores;
    let half = (cores.len() + 1) / 2; // Left column gets ceil(n/2) cores

    // Split into left and right columns (htop classic layout)
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_col = columns[0];
    let right_col = columns[1];

    // Left column: first half of CPU cores + Mem + Swp + Net
    let left_rows = half + 3; // cpu cores + mem + swap + net
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

    let cs = &app.color_scheme;

    // --- Left column: CPU cores 0..half ---
    for i in 0..half {
        if i < cores.len() {
            draw_cpu_bar(f, &cores[i], left_chunks[i], cs, app.cpu_count_from_zero);
        }
    }

    // --- Left column: Mem bar ---
    draw_memory_bar(f, app, left_chunks[half]);

    // --- Left column: Swap bar ---
    draw_swap_bar(f, app, left_chunks[half + 1]);

    // --- Left column: Network bar ---
    draw_network_bar(f, app, left_chunks[half + 2]);

    // --- Right column: CPU cores half..end ---
    for i in 0..right_cpu_count {
        let core_idx = half + i;
        if core_idx < cores.len() {
            draw_cpu_bar(f, &cores[core_idx], right_chunks[i], cs, app.cpu_count_from_zero);
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

/// Compact header for small screens/mobile: 1 aggregate CPU bar + 1 Mem bar
fn draw_compact_header(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Aggregate CPU bar
    let cores = &app.cpu_info.cores;
    let avg_usage: f64 = if cores.is_empty() {
        0.0
    } else {
        cores.iter().map(|c| c.usage_percent as f64).sum::<f64>() / cores.len() as f64
    };
    let cs = &app.color_scheme;
    let label = format!("CPU[{}]", cores.len());
    let pct_label = format!("{:>5.1}%", avg_usage);
    let bar_width = rows[0].width as usize;
    let available = bar_width.saturating_sub(label.len() + pct_label.len() + 3);
    let total_filled = ((avg_usage / 100.0) * available as f64) as usize;
    let total_filled = total_filled.min(available);
    let green_portion = (total_filled as f64 * 0.7) as usize;
    let red_portion = total_filled.saturating_sub(green_portion);
    let empty = available.saturating_sub(total_filled);
    let line = Line::from(vec![
        Span::styled(&label, Style::default().fg(cs.cpu_label).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(green_portion), Style::default().fg(cs.cpu_bar_normal)),
        Span::styled("|".repeat(red_portion), Style::default().fg(cs.cpu_bar_system)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(pct_label, Style::default().fg(cs.cpu_label)),
    ]);
    f.render_widget(Paragraph::new(line), rows[0]);

    // Memory bar (reuse existing logic inline for compactness)
    draw_memory_bar(f, app, rows[1]);
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
fn draw_cpu_bar(f: &mut Frame, core: &crate::system::cpu::CpuCore, area: Rect, cs: &crate::color_scheme::ColorScheme, cpu_from_zero: bool) {
    let usage = core.usage_percent;
    let display_id = if cpu_from_zero { core.id } else { core.id + 1 };
    let label = format!("{:>2}", display_id);
    let pct_label = format!("{:>5.1}%", usage);

    let bar_width = area.width as usize;
    let prefix_len = label.len() + 1;
    let suffix_len = pct_label.len() + 1;
    let bracket_len = 2;
    let available = bar_width.saturating_sub(prefix_len + suffix_len + bracket_len);

    let total_filled = ((usage as f64 / 100.0) * available as f64) as usize;
    let total_filled = total_filled.min(available);

    let green_portion = (total_filled as f64 * 0.7) as usize;
    let red_portion = total_filled.saturating_sub(green_portion);
    let empty = available.saturating_sub(total_filled);

    let line = Line::from(vec![
        Span::styled(
            format!("{} ", label),
            Style::default().fg(cs.cpu_label).add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(green_portion), Style::default().fg(cs.cpu_bar_normal)),
        Span::styled("|".repeat(red_portion), Style::default().fg(cs.cpu_bar_system)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(pct_label, Style::default().fg(cs.cpu_label)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw the memory usage bar with htop's multi-color scheme:
///   Green  = used memory pages
///   Blue   = buffer pages
///   Yellow = cache pages
fn draw_memory_bar(f: &mut Frame, app: &App, area: Rect) {
    let mem = &app.memory_info;
    let cs = &app.color_scheme;
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
        Span::styled(prefix, Style::default().fg(cs.cpu_label).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(green_len), Style::default().fg(cs.mem_bar_used)),
        Span::styled("|".repeat(blue_len), Style::default().fg(cs.mem_bar_buffers)),
        Span::styled("|".repeat(yellow_len), Style::default().fg(cs.mem_bar_cache)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(suffix, Style::default().fg(cs.cpu_label)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw the swap usage bar (green only, like htop)
fn draw_swap_bar(f: &mut Frame, app: &App, area: Rect) {
    let mem = &app.memory_info;
    let cs = &app.color_scheme;
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

    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(cs.cpu_label).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(filled), Style::default().fg(cs.swap_bar)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(suffix, Style::default().fg(cs.cpu_label)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw network throughput bar: "Net[||||rx|||tx| 1.2M/s↓ 340K/s↑]"
fn draw_network_bar(f: &mut Frame, app: &App, area: Rect) {
    let net = &app.network_info;

    let rx_str = format_rate(net.rx_bytes_per_sec);
    let tx_str = format_rate(net.tx_bytes_per_sec);
    let suffix = format!("{}↓ {}↑", rx_str, tx_str);

    let prefix = "Net";
    let bar_width = area.width as usize;
    let bracket_len = 2;
    let available = bar_width.saturating_sub(prefix.len() + suffix.len() + bracket_len + 1);

    // Scale bar based on a dynamic max (auto-scale to peak)
    let total_rate = net.rx_bytes_per_sec + net.tx_bytes_per_sec;
    // Use 1 Gbps as visual max for the bar
    let max_rate = 125_000_000.0_f64; // 1 Gbps in bytes/sec

    let rx_frac = if total_rate > 0.0 { net.rx_bytes_per_sec / max_rate } else { 0.0 };
    let tx_frac = if total_rate > 0.0 { net.tx_bytes_per_sec / max_rate } else { 0.0 };

    let green_len = ((rx_frac) * available as f64).min(available as f64) as usize;
    let magenta_len = ((tx_frac) * available as f64).min((available - green_len) as f64) as usize;
    let total_filled = (green_len + magenta_len).min(available);
    let empty = available.saturating_sub(total_filled);

    let cs = &app.color_scheme;
    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(cs.cpu_label).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(green_len), Style::default().fg(cs.cpu_bar_normal)),
        Span::styled("|".repeat(magenta_len), Style::default().fg(Color::Magenta)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(suffix, Style::default().fg(cs.cpu_label)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Format bytes/sec as human-readable rate
fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_073_741_824.0 {
        format!("{:.1} G/s", bytes_per_sec / 1_073_741_824.0)
    } else if bytes_per_sec >= 1_048_576.0 {
        format!("{:.1} M/s", bytes_per_sec / 1_048_576.0)
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.1} K/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

/// Draw: "Tasks: 312, 1024 thr; 5 running"
fn draw_tasks_line(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let line = Line::from(vec![
        Span::styled("Tasks: ", Style::default().fg(cs.info_label).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}", app.total_tasks), Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
        Span::styled(", ".to_string(), Style::default().fg(cs.info_value)),
        Span::styled(format!("{}", app.total_threads), Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
        Span::styled(" thr; ", Style::default().fg(cs.info_value)),
        Span::styled(format!("{}", app.running_tasks), Style::default().fg(cs.col_status_running).add_modifier(Modifier::BOLD)),
        Span::styled(" running", Style::default().fg(cs.info_value)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Draw: "Load average: 0.28 0.45 0.47"
fn draw_load_line(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let line = Line::from(vec![
        Span::styled("Load average: ", Style::default().fg(cs.info_label).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:.2} ", app.load_avg_1),
            Style::default().fg(if app.load_avg_1 > app.cpu_info.cores.len() as f64 { cs.col_cpu_high } else { cs.info_value }).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{:.2} ", app.load_avg_5), Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:.2}", app.load_avg_15), Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Draw: "Uptime: 05:12:01"
fn draw_uptime_line(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let line = Line::from(vec![
        Span::styled("Uptime: ", Style::default().fg(cs.info_label).add_modifier(Modifier::BOLD)),
        Span::styled(format_uptime(app.uptime_seconds), Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
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
