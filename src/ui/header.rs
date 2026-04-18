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
///     0[||||       25.3%]           4[||||||       42.1%]
///     1[||||||     43.2%]           5[||||         30.0%]
///     2[|||        18.0%]           6[|||||        35.2%]
///     3[|||||      33.0%]           7[|||          22.1%]
///   Mem[||||used|||cache|    5.2G/16.0G]    Tasks: 312, 1024 thr; 5 running
///   Swp[||               0.8G/8.0G]         Load average: 0.28 0.45 0.47
///   Net[||||rx|||tx| 1.2M/s↓ 340K/s↑]      Uptime: 05:12:01
///
/// On GPU tab, left column replaces Swap+Net with GPU+VMem:
///   Mem[||||used|||cache|    5.2G/16.0G]
///   GPU[||||||||       45.2%]
///   VMem[||||      2.1G used]
pub fn draw_header(f: &mut Frame, app: &App, area: Rect, term_height: u16, term_width: u16) {
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

    // header_margin: add horizontal AND vertical padding when enabled
    let content_area = if app.header_margin {
        Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    } else {
        area
    };

    // Determine which panels have CPU bars based on configured meters
    let left_has_cpus = app.left_meters.iter().any(|m| is_cpu_meter(m));
    let right_has_cpus = app.right_meters.iter().any(|m| is_cpu_meter(m));

    // Collect non-CPU info meters for each panel
    let left_info: Vec<&str> = app.left_meters.iter()
        .filter(|m| !is_cpu_meter(m))
        .map(|s| s.as_str())
        .collect();
    let right_info: Vec<&str> = app.right_meters.iter()
        .filter(|m| !is_cpu_meter(m))
        .map(|s| s.as_str())
        .collect();

    // Use the actual terminal dimensions (same as header_height) to ensure
    // cpu_column_count returns the same value, avoiding height mismatches.
    let auto_cpu_cols = super::cpu_column_count(core_count, term_height, term_width);
    let auto_sub_cols = (auto_cpu_cols / 2).max(1);

    // Check if either panel uses an explicit CPU meter variant (AllCPUs2/4/8)
    let left_cpu_meter = app.left_meters.iter().find(|m| is_cpu_meter(m));
    let right_cpu_meter = app.right_meters.iter().find(|m| is_cpu_meter(m));

    // Per-panel sub-column counts: use explicit variant if set, otherwise auto
    let left_sub_cols = left_cpu_meter
        .and_then(|m| cpu_meter_subcols(m))
        .unwrap_or(auto_sub_cols);
    let right_sub_cols = right_cpu_meter
        .and_then(|m| cpu_meter_subcols(m))
        .unwrap_or(auto_sub_cols);

    let (left_cores_start, left_cores_count, right_cores_start, right_cores_count) =
        if left_has_cpus && right_has_cpus {
            let half = (core_count + 1) / 2;
            (0, half, half, core_count - half)
        } else if left_has_cpus {
            (0, core_count, 0, 0)
        } else if right_has_cpus {
            (0, 0, 0, core_count)
        } else {
            (0, 0, 0, 0)
        };

    let left_cpu_rows = if left_cores_count > 0 {
        (left_cores_count + left_sub_cols - 1) / left_sub_cols
    } else {
        0
    };
    let right_cpu_rows = if right_cores_count > 0 {
        (right_cores_count + right_sub_cols - 1) / right_sub_cols
    } else {
        0
    };

    let left_total = left_cpu_rows + left_info.len();
    let right_total = right_cpu_rows + right_info.len();

    // Split into left and right panels with 1-char separator (htop style)
    // htop Header_draw: width = COLS - 2*pad - (numCols-1), each col = width * pct / 100
    // For 2-col 50/50: left = (w-1)/2, right = (w-1) - left (right gets the extra pixel)
    let usable = content_area.width.saturating_sub(1); // exclude 1-char separator
    let left_w = usable / 2;
    let right_w = usable - left_w;
    let left_panel = Rect {
        x: content_area.x,
        y: content_area.y,
        width: left_w,
        height: content_area.height,
    };
    let right_panel = Rect {
        x: content_area.x + left_w + 1,
        y: content_area.y,
        width: right_w,
        height: content_area.height,
    };

    // --- LEFT PANEL ---
    if left_total > 0 {
        let panel = left_panel;
        let row_constraints: Vec<Constraint> = (0..left_total)
            .map(|_| Constraint::Length(1))
            .collect();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(panel);

        // CPU bars
        render_cpu_bars(f, app, left_cores_start, left_cores_count, left_sub_cols, &rows, left_cpu_rows);

        // Info meters
        for (i, meter_name) in left_info.iter().enumerate() {
            let row_idx = left_cpu_rows + i;
            if row_idx < rows.len() {
                draw_meter(f, app, meter_name, rows[row_idx]);
            }
        }
    }

    // --- RIGHT PANEL ---
    if right_total > 0 {
        let panel = right_panel;
        let row_constraints: Vec<Constraint> = (0..right_total)
            .map(|_| Constraint::Length(1))
            .collect();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(panel);

        // CPU bars
        render_cpu_bars(f, app, right_cores_start, right_cores_count, right_sub_cols, &rows, right_cpu_rows);

        // Info meters
        for (i, meter_name) in right_info.iter().enumerate() {
            let row_idx = right_cpu_rows + i;
            if row_idx < rows.len() {
                draw_meter(f, app, meter_name, rows[row_idx]);
            }
        }
    }
}

/// Check if a meter name represents CPU bars
fn is_cpu_meter(name: &str) -> bool {
    matches!(name, "AllCPUs" | "AllCPUs2" | "AllCPUs4" | "AllCPUs8") || name.starts_with("CPUs")
}

/// Get the forced sub-column count for a specific CPU meter variant.
/// Returns None for "AllCPUs" (auto-detect) and a fixed count for explicit variants.
fn cpu_meter_subcols(name: &str) -> Option<usize> {
    match name {
        "AllCPUs2" => Some(2),  // htop AllCPUs2: 2 sub-columns per panel
        "AllCPUs4" => Some(4),  // htop AllCPUs4: 4 sub-columns per panel
        "AllCPUs8" => Some(8),  // htop AllCPUs8: 8 sub-columns per panel
        _ => None,              // AllCPUs: auto-detect
    }
}

/// Render CPU bars for a range of cores into the given rows
fn render_cpu_bars(
    f: &mut Frame,
    app: &App,
    cores_start: usize,
    cores_count: usize,
    sub_cols_per_panel: usize,
    rows: &[Rect],
    max_rows: usize,
) {
    if cores_count == 0 || max_rows == 0 {
        return;
    }
    let cores = &app.cpu_info.cores;
    let cs = &app.color_scheme;
    let cores_per_sub = (cores_count + sub_cols_per_panel - 1) / sub_cols_per_panel;

    if sub_cols_per_panel == 1 {
        for i in 0..cores_count.min(max_rows) {
            let core_idx = cores_start + i;
            if core_idx < cores.len() && i < rows.len() {
                draw_cpu_bar(f, &cores[core_idx], rows[i], cs, app.cpu_count_from_zero,
                    app.cpu_user_frac, app.cpu_kernel_frac, app.detailed_cpu_time);
            }
        }
    } else {
        for row_i in 0..max_rows {
            if row_i >= rows.len() { break; }
            let sub_constraints: Vec<Constraint> = (0..sub_cols_per_panel)
                .map(|_| Constraint::Ratio(1, sub_cols_per_panel as u32))
                .collect();
            let sub_cells = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(sub_constraints)
                .split(rows[row_i]);

            for sub_i in 0..sub_cols_per_panel {
                let core_idx = cores_start + sub_i * cores_per_sub + row_i;
                if core_idx < cores_start + cores_count && core_idx < cores.len() && sub_i < sub_cells.len() {
                    draw_cpu_bar(f, &cores[core_idx], sub_cells[sub_i], cs,
                        app.cpu_count_from_zero, app.cpu_user_frac, app.cpu_kernel_frac,
                        app.detailed_cpu_time);
                }
            }
        }
    }
}

/// Dispatch a single meter by name to its rendering function
fn draw_meter(f: &mut Frame, app: &App, name: &str, area: Rect) {
    match name {
        "Memory" => draw_memory_bar(f, app, area),
        "Swap" => {
            if app.active_tab == ProcessTab::Gpu {
                draw_gpu_bar(f, app, area);
            } else {
                draw_swap_bar(f, app, area);
            }
        }
        "Network" => {
            if app.active_tab == ProcessTab::Gpu {
                draw_vram_bar(f, app, area);
            } else {
                draw_network_bar(f, app, area);
            }
        }
        "GPU" => draw_gpu_bar(f, app, area),
        "VMem" => draw_vram_bar(f, app, area),
        "Tasks" => draw_tasks_line(f, app, area),
        "Load average" => draw_load_line(f, app, area),
        "Uptime" => draw_uptime_line(f, app, area),
        "CPU average" => draw_cpu_average_bar(f, app, area),
        "Clock" => draw_clock_line(f, app, area),
        "Hostname" => draw_hostname_line(f, app, area),
        "Blank" => {} // empty row
        _ => {} // unknown meter, skip
    }
}

/// Compact header for small screens/mobile: 1 aggregate CPU bar + 1 Mem bar
fn draw_compact_header(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Aggregate CPU bar using averaged per-core fractions
    let cores = &app.cpu_info.cores;
    let cs = &app.color_scheme;
    let n = cores.len().max(1) as f64;

    let avg_user = cores.iter().map(|c| c.user_frac as f64).sum::<f64>() / n;
    let avg_kernel = cores.iter().map(|c| c.kernel_frac as f64).sum::<f64>() / n;
    let avg_dpc = cores.iter().map(|c| c.dpc_frac as f64).sum::<f64>() / n;
    let avg_interrupt = cores.iter().map(|c| c.interrupt_frac as f64).sum::<f64>() / n;

    let avg_usage = cores.iter().map(|c| c.usage_percent as f64).sum::<f64>() / n;
    let text = format!("{:.1}%", avg_usage);

    if app.detailed_cpu_time {
        draw_htop_bar(
            f,
            "CPU",
            &[
                (avg_user, cs.cpu_bar_normal),
                (avg_kernel, cs.cpu_bar_system),
                (avg_interrupt, cs.cpu_bar_irq),
                (avg_dpc, cs.cpu_bar_softirq),
            ],
            &text,
            cs.cpu_label,
            cs.cpu_bar_bg,
            rows[0],
        );
    } else {
        let sys = avg_kernel + avg_dpc + avg_interrupt;
        draw_htop_bar(
            f,
            "CPU",
            &[(avg_user, cs.cpu_bar_normal), (sys, cs.cpu_bar_system)],
            &text,
            cs.cpu_label,
            cs.cpu_bar_bg,
            rows[0],
        );
    }

    // Memory bar (reuse existing logic inline for compactness)
    draw_memory_bar(f, app, rows[1]);
}

/// Render an htop-style bar meter: `Cap[||||||||       text]`
/// Text is right-aligned inside the brackets; bar fills from left, only in spaces.
/// Characters in the filled area get the segment color; empty area gets shadow color.
fn draw_htop_bar(
    f: &mut Frame,
    caption: &str,
    segments: &[(f64, Color)],
    text: &str,
    label_color: Color,
    shadow_color: Color,
    area: Rect,
) {
    let w = area.width as usize;
    if w == 0 {
        return;
    }
    let cap_len = caption.len();
    if w <= cap_len + 2 {
        return;
    }
    let inner_w = w - cap_len - 2;

    // Build character buffer (all spaces initially)
    let mut chars: Vec<char> = vec![' '; inner_w];
    let mut colors: Vec<Color> = vec![shadow_color; inner_w];

    // Place text right-aligned inside the bar area
    let text_chars: Vec<char> = text.chars().collect();
    let text_len = text_chars.len().min(inner_w);
    let text_start = inner_w.saturating_sub(text_len);
    let text_offset = if text_chars.len() > inner_w {
        text_chars.len() - inner_w
    } else {
        0
    };
    for i in 0..text_len {
        chars[text_start + i] = text_chars[text_offset + i];
        colors[text_start + i] = label_color;
    }

    // Fill bar segments from left; only replace space chars with '|'
    let mut offset = 0;
    for &(frac, color) in segments {
        let fill = (frac * inner_w as f64).round() as usize;
        let fill = fill.min(inner_w.saturating_sub(offset));
        for i in offset..offset + fill {
            if chars[i] == ' ' {
                chars[i] = '|';
            }
            colors[i] = color;
        }
        offset += fill;
    }

    // Group consecutive characters with same color into spans
    let mut spans = vec![
        Span::styled(
            caption.to_string(),
            Style::default().fg(label_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(label_color)),
    ];

    if inner_w > 0 {
        let mut run = String::new();
        let mut run_color = colors[0];
        for i in 0..inner_w {
            if colors[i] == run_color {
                run.push(chars[i]);
            } else {
                spans.push(Span::styled(run.clone(), Style::default().fg(run_color)));
                run.clear();
                run_color = colors[i];
                run.push(chars[i]);
            }
        }
        if !run.is_empty() {
            spans.push(Span::styled(run, Style::default().fg(run_color)));
        }
    }

    spans.push(Span::styled("]", Style::default().fg(label_color)));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Draw a single CPU core usage bar with htop's multi-color scheme.
///
/// Basic mode (htop default):
///   Green  = user processes
///   Red    = kernel (includes DPC + interrupt, like htop bundles irq/softirq into sys)
///
/// Detailed mode (htop detailed_cpu_time):
///   Green   = user processes
///   Red     = pure kernel
///   Yellow  = interrupt time (≈ htop IRQ)
///   Magenta = DPC time (≈ htop softIRQ)
///
/// Uses per-core time fractions from NtQuerySystemInformation for accurate display.
fn draw_cpu_bar(f: &mut Frame, core: &crate::system::cpu::CpuCore, area: Rect, cs: &crate::color_scheme::ColorScheme, cpu_from_zero: bool, _user_frac: f64, _kernel_frac: f64, detailed: bool) {
    let display_id = if cpu_from_zero { core.id } else { core.id + 1 };
    let caption = format!("{:>3}", display_id);
    let text = format!("{:.1}%", core.usage_percent);

    if detailed {
        // Detailed mode: show user + kernel + interrupt + DPC as separate segments
        // Per-core fracs are fractions of total time (including idle)
        draw_htop_bar(
            f,
            &caption,
            &[
                (core.user_frac as f64, cs.cpu_bar_normal),       // green: user
                (core.kernel_frac as f64, cs.cpu_bar_system),     // red: pure kernel
                (core.interrupt_frac as f64, cs.cpu_bar_irq),     // yellow: interrupt (≈ irq)
                (core.dpc_frac as f64, cs.cpu_bar_softirq),       // magenta: DPC (≈ softirq)
            ],
            &text,
            cs.cpu_label,
            cs.cpu_bar_bg,
            area,
        );
    } else {
        // Basic mode: user (green) + kernel including DPC+interrupt (red)
        // This matches htop basic mode where sys = kernel + irq + softirq
        let user = core.user_frac as f64;
        let sys = core.kernel_frac as f64 + core.dpc_frac as f64 + core.interrupt_frac as f64;
        draw_htop_bar(
            f,
            &caption,
            &[
                (user, cs.cpu_bar_normal),  // green: user
                (sys, cs.cpu_bar_system),   // red: all kernel-mode
            ],
            &text,
            cs.cpu_label,
            cs.cpu_bar_bg,
            area,
        );
    }
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
    let text = format!("{}/{}", used_str, total_str);

    draw_htop_bar(
        f,
        "Mem",
        &[(used_frac, cs.mem_bar_used), (buffer_frac, cs.mem_bar_buffers), (cache_frac, cs.mem_bar_cache)],
        &text,
        cs.cpu_label,
        cs.cpu_bar_bg,
        area,
    );
}

/// Draw the swap usage bar (green only, like htop)
fn draw_swap_bar(f: &mut Frame, app: &App, area: Rect) {
    let mem = &app.memory_info;
    let cs = &app.color_scheme;
    let total = mem.total_swap as f64;
    let usage_frac = if total > 0.0 { mem.used_swap as f64 / total } else { 0.0 };

    let used_str = format_bytes(mem.used_swap);
    let total_str = format_bytes(mem.total_swap);
    let text = format!("{}/{}", used_str, total_str);

    draw_htop_bar(
        f,
        "Swp",
        &[(usage_frac, cs.swap_bar)],
        &text,
        cs.cpu_label,
        cs.cpu_bar_bg,
        area,
    );
}

/// Draw network throughput bar: "Net[||||rx|||tx|   ↓1.2M/s ↑340K/s]"
fn draw_network_bar(f: &mut Frame, app: &App, area: Rect) {
    let net = &app.network_info;
    let cs = &app.color_scheme;

    let rx_str = format_rate(net.rx_bytes_per_sec);
    let tx_str = format_rate(net.tx_bytes_per_sec);
    let text = format!("↓{} ↑{}", rx_str, tx_str);

    // Scale bar based on 1 Gbps as visual max
    let max_rate = 125_000_000.0_f64;
    let rx_frac = if net.rx_bytes_per_sec > 0.0 { (net.rx_bytes_per_sec / max_rate).min(1.0) } else { 0.0 };
    let tx_frac = if net.tx_bytes_per_sec > 0.0 { (net.tx_bytes_per_sec / max_rate).min(1.0) } else { 0.0 };

    draw_htop_bar(
        f,
        "Net",
        &[(rx_frac, cs.cpu_bar_normal), (tx_frac, Color::Magenta)],
        &text,
        cs.cpu_label,
        cs.cpu_bar_bg,
        area,
    );
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

/// Format uptime as DD days, HH:MM:SS (matching htop UptimeMeter.c exactly)
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 100 {
        format!("{} days(!), {:02}:{:02}:{:02}", days, hours, minutes, secs)
    } else if days > 1 {
        format!("{} days, {:02}:{:02}:{:02}", days, hours, minutes, secs)
    } else if days == 1 {
        format!("1 day, {:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }
}

/// Draw GPU utilization bar: "GPU[||||||||       45.2%]"
fn draw_gpu_bar(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let usage = app.gpu_overall_usage;
    let usage_frac = (usage / 100.0).clamp(0.0, 1.0) as f64;

    let text = format!("{:.1}%", usage);

    // Color the bar: green < 50%, yellow 50-80%, red > 80%
    let bar_color = if usage > 80.0 {
        Color::Red
    } else if usage > 50.0 {
        Color::Yellow
    } else {
        cs.cpu_bar_normal
    };

    draw_htop_bar(
        f,
        "GPU",
        &[(usage_frac, bar_color)],
        &text,
        Color::LightCyan,
        cs.cpu_bar_bg,
        area,
    );
}

/// Draw GPU VRAM bar: "VMem[||||      2.1G used]"
fn draw_vram_bar(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let dedicated = app.gpu_dedicated_mem;

    let used_str = format_bytes(dedicated);
    let text = format!("{} used", used_str);

    // Scale against a reasonable GPU VRAM max — auto-detect would be ideal,
    // but for now use 24 GB as a reasonable modern GPU ceiling.
    let vram_max: u64 = 24 * 1024 * 1024 * 1024;
    let usage_frac = if vram_max > 0 {
        (dedicated as f64 / vram_max as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let bar_color = if usage_frac > 0.8 {
        Color::Red
    } else if usage_frac > 0.5 {
        Color::Yellow
    } else {
        Color::LightCyan
    };

    draw_htop_bar(
        f,
        "VMem",
        &[(usage_frac, bar_color)],
        &text,
        Color::LightCyan,
        cs.cpu_bar_bg,
        area,
    );
}

/// Draw a single aggregate CPU average bar
fn draw_cpu_average_bar(f: &mut Frame, app: &App, area: Rect) {
    let cores = &app.cpu_info.cores;
    let cs = &app.color_scheme;
    let n = cores.len().max(1) as f64;

    let avg_user = cores.iter().map(|c| c.user_frac as f64).sum::<f64>() / n;
    let avg_kernel = cores.iter().map(|c| c.kernel_frac as f64).sum::<f64>() / n;
    let avg_dpc = cores.iter().map(|c| c.dpc_frac as f64).sum::<f64>() / n;
    let avg_interrupt = cores.iter().map(|c| c.interrupt_frac as f64).sum::<f64>() / n;

    let avg_usage = cores.iter().map(|c| c.usage_percent as f64).sum::<f64>() / n;
    let text = format!("{:.1}%", avg_usage);

    if app.detailed_cpu_time {
        draw_htop_bar(
            f,
            "CPU",
            &[
                (avg_user, cs.cpu_bar_normal),
                (avg_kernel, cs.cpu_bar_system),
                (avg_interrupt, cs.cpu_bar_irq),
                (avg_dpc, cs.cpu_bar_softirq),
            ],
            &text,
            cs.cpu_label,
            cs.cpu_bar_bg,
            area,
        );
    } else {
        let sys = avg_kernel + avg_dpc + avg_interrupt;
        draw_htop_bar(
            f,
            "CPU",
            &[(avg_user, cs.cpu_bar_normal), (sys, cs.cpu_bar_system)],
            &text,
            cs.cpu_label,
            cs.cpu_bar_bg,
            area,
        );
    }
}

/// Draw clock: "Clock: HH:MM:SS"
fn draw_clock_line(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let now = chrono::Local::now();
    let time_str = now.format("%H:%M:%S").to_string();
    let line = Line::from(vec![
        Span::styled("Clock: ", Style::default().fg(cs.info_label).add_modifier(Modifier::BOLD)),
        Span::styled(time_str, Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Draw hostname: "Hostname: COMPUTERNAME"
fn draw_hostname_line(f: &mut Frame, app: &App, area: Rect) {
    let cs = &app.color_scheme;
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    let line = Line::from(vec![
        Span::styled("Hostname: ", Style::default().fg(cs.info_label).add_modifier(Modifier::BOLD)),
        Span::styled(hostname, Style::default().fg(cs.info_value).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
