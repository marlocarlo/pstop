use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ProcessTab};
use crate::system::memory::format_bytes;

/// Draw the complete header area in htop's exact layout:
///
/// Each column flows independently — info meters appear immediately
/// after the last CPU bar in that column, NOT force-aligned across panels.
///
/// LEFT COLUMN (50%):            RIGHT COLUMN (50%):
///   0 [||||     25.3%]            4 [||||||     42.1%]
///   1 [||||||   43.2%]            5 [||||       30.0%]
///   2 [|||      18.0%]            6 [|||||      35.2%]
///   3 [|||||    33.0%]            7 [|||        22.1%]
///   Mem[||||used|||cache|    5.2G/16.0G]    Tasks: 312, 1024 thr; 5 running
///   Swp[||               0.8G/8.0G]         Load average: 0.28 0.45 0.47
///   Net[||||rx|||tx| 1.2M/s↓ 340K/s↑]      Uptime: 05:12:01
///
/// On GPU tab, left column replaces Swap+Net with GPU+VMem:
///   Mem[||||used|||cache|    5.2G/16.0G]
///   GPU[||||||||       45.2%]
///   VMem[||||      2.1G used]
pub fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    // Compact mode: single aggregate CPU bar + memory bar
    if app.compact_mode {
        draw_compact_header(f, app, area);
        return;
    }

    let cores = &app.cpu_info.cores;
    let core_count = cores.len();
    if core_count == 0 {
        return;
    }

    // header_margin: add horizontal padding when enabled
    let content_area = if app.header_margin {
        Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        }
    } else {
        area
    };

    // Calculate optimal CPU column count (2, 4, 8, 16) — htop-style auto-alignment
    let cpu_cols = {
        let max_cpu_rows = (area.height as usize).saturating_sub(3);
        if max_cpu_rows == 0 {
            2usize
        } else {
            let max_by_width = (content_area.width / super::MIN_CPU_COL_WIDTH).max(2) as usize;
            let mut result = 2usize;
            for &cols in &[2, 4, 8, 16] {
                if cols > max_by_width { break; }
                let rows_needed = (core_count + cols - 1) / cols;
                if rows_needed <= max_cpu_rows {
                    result = cols;
                    break;
                }
                result = cols;
            }
            result
        }
    };

    let cs = &app.color_scheme;

    // CPU distribution: first half goes to left panel, rest to right panel
    let sub_cols_per_panel = (cpu_cols / 2).max(1);
    let half = (core_count + 1) / 2;
    let cores_per_sub_left = (half + sub_cols_per_panel - 1) / sub_cols_per_panel;
    let right_core_count = core_count - half;
    let cores_per_sub_right = if right_core_count > 0 {
        (right_core_count + sub_cols_per_panel - 1) / sub_cols_per_panel
    } else {
        0
    };

    // htop-style: each column flows independently
    let left_cpu_rows = cores_per_sub_left;
    let right_cpu_rows = cores_per_sub_right;
    let left_info_count = 3; // Mem + Swap/GPU + Net/VMem
    let right_info_count = 3; // Tasks + Load + Uptime
    let left_total = left_cpu_rows + left_info_count;
    let right_total = right_cpu_rows + right_info_count;

    // Split into left and right panels (50/50)
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(content_area);

    // --- LEFT PANEL ---
    {
        let panel = panels[0];
        let row_constraints: Vec<Constraint> = (0..left_total)
            .map(|_| Constraint::Length(1))
            .collect();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(panel);

        // CPU bars
        if sub_cols_per_panel == 1 {
            for i in 0..half.min(left_cpu_rows) {
                if i < cores.len() && i < rows.len() {
                    draw_cpu_bar(f, &cores[i], rows[i], cs, app.cpu_count_from_zero,
                        app.cpu_user_frac, app.cpu_kernel_frac, app.detailed_cpu_time);
                }
            }
        } else {
            for row_i in 0..left_cpu_rows {
                if row_i >= rows.len() { break; }
                let sub_constraints: Vec<Constraint> = (0..sub_cols_per_panel)
                    .map(|_| Constraint::Ratio(1, sub_cols_per_panel as u32))
                    .collect();
                let sub_cells = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(sub_constraints)
                    .split(rows[row_i]);

                for sub_i in 0..sub_cols_per_panel {
                    let core_idx = sub_i * cores_per_sub_left + row_i;
                    if core_idx < half && core_idx < cores.len() && sub_i < sub_cells.len() {
                        draw_cpu_bar(f, &cores[core_idx], sub_cells[sub_i], cs,
                            app.cpu_count_from_zero, app.cpu_user_frac, app.cpu_kernel_frac,
                            app.detailed_cpu_time);
                    }
                }
            }
        }

        // Info rows immediately after last CPU row (htop-style: no gap)
        let info_start = left_cpu_rows;
        if info_start < rows.len() {
            draw_memory_bar(f, app, rows[info_start]);
        }
        if info_start + 1 < rows.len() {
            if app.active_tab == ProcessTab::Gpu {
                draw_gpu_bar(f, app, rows[info_start + 1]);
            } else {
                draw_swap_bar(f, app, rows[info_start + 1]);
            }
        }
        if info_start + 2 < rows.len() {
            if app.active_tab == ProcessTab::Gpu {
                draw_vram_bar(f, app, rows[info_start + 2]);
            } else {
                draw_network_bar(f, app, rows[info_start + 2]);
            }
        }
    }

    // --- RIGHT PANEL ---
    {
        let panel = panels[1];
        let row_constraints: Vec<Constraint> = (0..right_total)
            .map(|_| Constraint::Length(1))
            .collect();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(panel);

        // CPU bars
        if sub_cols_per_panel == 1 {
            for i in 0..right_core_count.min(right_cpu_rows) {
                let core_idx = half + i;
                if core_idx < cores.len() && i < rows.len() {
                    draw_cpu_bar(f, &cores[core_idx], rows[i], cs, app.cpu_count_from_zero,
                        app.cpu_user_frac, app.cpu_kernel_frac, app.detailed_cpu_time);
                }
            }
        } else {
            for row_i in 0..right_cpu_rows {
                if row_i >= rows.len() { break; }
                let sub_constraints: Vec<Constraint> = (0..sub_cols_per_panel)
                    .map(|_| Constraint::Ratio(1, sub_cols_per_panel as u32))
                    .collect();
                let sub_cells = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(sub_constraints)
                    .split(rows[row_i]);

                for sub_i in 0..sub_cols_per_panel {
                    let core_idx = half + sub_i * cores_per_sub_right + row_i;
                    if core_idx < cores.len() && sub_i < sub_cells.len() {
                        draw_cpu_bar(f, &cores[core_idx], sub_cells[sub_i], cs,
                            app.cpu_count_from_zero, app.cpu_user_frac, app.cpu_kernel_frac,
                            app.detailed_cpu_time);
                    }
                }
            }
        }

        // Info rows immediately after last CPU row (htop-style: no gap)
        let info_start = right_cpu_rows;
        if info_start < rows.len() {
            draw_tasks_line(f, app, rows[info_start]);
        }
        if info_start + 1 < rows.len() {
            draw_load_line(f, app, rows[info_start + 1]);
        }
        if info_start + 2 < rows.len() {
            draw_uptime_line(f, app, rows[info_start + 2]);
        }
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
    // Use real user/kernel split when detailed_cpu_time is on
    let (green_portion, red_portion) = if app.detailed_cpu_time {
        let total = app.cpu_user_frac + app.cpu_kernel_frac;
        if total > 0.0 {
            let u = (app.cpu_user_frac / total * total_filled as f64) as usize;
            let k = total_filled.saturating_sub(u);
            (u, k)
        } else {
            (total_filled, 0)
        }
    } else {
        let g = (total_filled as f64 * 0.7) as usize;
        (g, total_filled.saturating_sub(g))
    };
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
/// When detailed_cpu_time is ON, uses real GetSystemTimes data for user/kernel split.
/// When OFF, uses a 70/30 visual approximation.
fn draw_cpu_bar(f: &mut Frame, core: &crate::system::cpu::CpuCore, area: Rect, cs: &crate::color_scheme::ColorScheme, cpu_from_zero: bool, user_frac: f64, kernel_frac: f64, detailed: bool) {
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

    // Use real user/kernel split from GetSystemTimes when detailed_cpu_time is on
    let (user_portion, kernel_portion) = if detailed {
        let total = user_frac + kernel_frac;
        if total > 0.0 {
            let u = (user_frac / total * total_filled as f64) as usize;
            let k = total_filled.saturating_sub(u);
            (u, k)
        } else {
            (total_filled, 0)
        }
    } else {
        let green_portion = (total_filled as f64 * 0.7) as usize;
        let red_portion = total_filled.saturating_sub(green_portion);
        (green_portion, red_portion)
    };
    let empty = available.saturating_sub(total_filled);

    let line = Line::from(vec![
        Span::styled(
            format!("{} ", label),
            Style::default().fg(cs.cpu_label).add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(user_portion), Style::default().fg(cs.cpu_bar_normal)),
        Span::styled("|".repeat(kernel_portion), Style::default().fg(cs.cpu_bar_system)),
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

/// Draw GPU utilization bar: "GPU[||||||||       45.2%]"
fn draw_gpu_bar(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let usage = app.gpu_overall_usage;
    let usage_frac = (usage / 100.0).clamp(0.0, 1.0);

    let suffix = format!("{:5.1}%", usage);
    let prefix = "GPU";
    let bar_width = area.width as usize;
    let bracket_len = 2;
    let available = bar_width.saturating_sub(prefix.len() + suffix.len() + bracket_len + 1);

    let filled = (usage_frac * available as f64) as usize;
    let filled = filled.min(available);
    let empty = available.saturating_sub(filled);

    // Color the bar: green < 50%, yellow 50-80%, red > 80%
    let bar_color = if usage > 80.0 {
        Color::Red
    } else if usage > 50.0 {
        Color::Yellow
    } else {
        cs.cpu_bar_normal
    };

    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(filled), Style::default().fg(bar_color)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(suffix, Style::default().fg(cs.cpu_label)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

/// Draw GPU VRAM bar: "VMem[||||      2.1G used]"
fn draw_vram_bar(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let dedicated = app.gpu_dedicated_mem;

    let used_str = format_bytes(dedicated);
    let suffix = format!("{} used", used_str);

    let prefix = "VMem";
    let bar_width = area.width as usize;
    let bracket_len = 2;
    let available = bar_width.saturating_sub(prefix.len() + suffix.len() + bracket_len + 1);

    // Scale against a reasonable GPU VRAM max — auto-detect would be ideal,
    // but for now use 24 GB as a reasonable modern GPU ceiling.
    let vram_max: u64 = 24 * 1024 * 1024 * 1024;
    let usage_frac = if vram_max > 0 {
        (dedicated as f64 / vram_max as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let filled = (usage_frac * available as f64) as usize;
    let filled = filled.min(available);
    let empty = available.saturating_sub(filled);

    let bar_color = if usage_frac > 0.8 {
        Color::Red
    } else if usage_frac > 0.5 {
        Color::Yellow
    } else {
        Color::LightCyan
    };

    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(cs.cpu_label)),
        Span::styled("|".repeat(filled), Style::default().fg(bar_color)),
        Span::styled(" ".repeat(empty), Style::default().fg(cs.cpu_bar_bg)),
        Span::styled("]", Style::default().fg(cs.cpu_label)),
        Span::styled(suffix, Style::default().fg(cs.cpu_label)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
