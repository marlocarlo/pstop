pub mod header;
pub mod process_table;
pub mod footer;
pub mod help;
pub mod sort_menu;
pub mod kill_menu;
pub mod user_menu;
pub mod affinity_menu;
pub mod environment_view;
pub mod setup_menu;
pub mod handles_view;
pub mod tab_bar;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::{App, AppMode};

/// Minimum width (chars) for a single CPU bar column to remain readable
const MIN_CPU_COL_WIDTH: u16 = 15;

/// Calculate the optimal number of CPU columns based on core count and terminal size.
/// Returns 2, 4, 8, or 16. Always even (left/right panel symmetry).
/// htop-style: uses more columns when core count is high relative to terminal height,
/// so the header never dominates the screen.
pub fn cpu_column_count(core_count: usize, term_height: u16, term_width: u16) -> usize {
    if core_count <= 1 {
        return 2;
    }

    // Max header height ≈ 40% of terminal, but at least 6 rows
    let max_header = ((term_height as usize) * 2 / 5).max(6);
    let max_cpu_rows = max_header.saturating_sub(3); // 3 rows for info meters (Mem/Swap/Net or Tasks/Load/Uptime)
    if max_cpu_rows == 0 {
        return 2;
    }

    // Max columns that fit horizontally (each column needs MIN_CPU_COL_WIDTH chars)
    let max_cols_by_width = (term_width / MIN_CPU_COL_WIDTH) as usize;
    let max_cols_by_width = max_cols_by_width.max(2);

    // Find smallest column count (powers of 2) where CPU rows fit
    for &cols in &[2, 4, 8, 16] {
        if cols > max_cols_by_width {
            // Can't fit this many columns horizontally; use previous
            break;
        }
        let rows_needed = (core_count + cols - 1) / cols;
        if rows_needed <= max_cpu_rows {
            return cols;
        }
    }

    // Fallback: use maximum feasible column count
    max_cols_by_width.min(16).max(2)
}

/// Calculate the header height based on number of CPU cores and terminal size.
/// htop-style: each panel flows independently, so height = max(left, right).
pub fn header_height(app: &App, term_height: u16, term_width: u16) -> u16 {
    if app.compact_mode {
        return 2; // 1 aggregate CPU bar + 1 Mem bar
    }
    let cores = app.cpu_info.cores.len();
    if cores == 0 {
        return 5; // fallback: just info rows
    }
    let cpu_cols = cpu_column_count(cores, term_height, term_width);
    let sub_cols_per_panel = (cpu_cols / 2).max(1);
    let half = (cores + 1) / 2;
    let right_count = cores - half;
    let left_cpu_rows = (half + sub_cols_per_panel - 1) / sub_cols_per_panel;
    let right_cpu_rows = if right_count > 0 {
        (right_count + sub_cols_per_panel - 1) / sub_cols_per_panel
    } else {
        0
    };
    let left_total = left_cpu_rows + 3; // Mem + Swap/GPU + Net/VMem
    let right_total = right_cpu_rows + 3; // Tasks + Load + Uptime
    let pad: usize = if app.header_margin { 2 } else { 0 }; // htop: pad=2 when margin on
    (left_total.max(right_total) + pad) as u16
}

/// Render the complete UI
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let h_height = header_height(app, size.height, size.width);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(h_height),   // header (CPU + mem + info)
            Constraint::Length(1),          // tab bar (Main | I/O)
            Constraint::Min(5),             // process table
            Constraint::Length(1),          // footer (F-key bar)
        ])
        .split(size);

    header::draw_header(f, app, chunks[0]);
    tab_bar::draw_tab_bar(f, app, chunks[1]);
    process_table::draw_process_table(f, app, chunks[2]);
    footer::draw_footer(f, app, chunks[3]);

    // Overlay popups
    match app.mode {
        AppMode::Help => help::draw_help(f),
        AppMode::Setup => setup_menu::draw_setup_menu(f, app),
        AppMode::SortSelect => sort_menu::draw_sort_menu(f, app),
        AppMode::Kill => kill_menu::draw_kill_menu(f, app),
        AppMode::UserFilter => user_menu::draw_user_menu(f, app),
        AppMode::Affinity => affinity_menu::draw_affinity_menu(f, app),
        AppMode::Environment => environment_view::draw_environment_view(f, app),
        AppMode::Handles => handles_view::draw_handles_view(f, app),
        _ => {}
    }
}
