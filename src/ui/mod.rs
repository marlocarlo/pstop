pub mod header;
pub mod process_table;
pub mod footer;
pub mod help;
pub mod sort_menu;
pub mod kill_menu;
pub mod user_menu;
pub mod affinity_menu;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::{App, AppMode};

/// Calculate the header height based on number of CPU cores (htop layout)
pub fn header_height(app: &App) -> u16 {
    let cores = app.cpu_info.cores.len();
    let half = (cores + 1) / 2;
    // Left column: half cores + Mem + Swap
    // Right column: other half cores + Tasks + Load + Uptime
    let left_rows = half + 2;
    let right_rows = (cores - half) + 3;
    left_rows.max(right_rows) as u16
}

/// Render the complete UI
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let h_height = header_height(app);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(h_height),   // header (CPU + mem + info)
            Constraint::Min(5),             // process table
            Constraint::Length(1),          // footer (F-key bar)
        ])
        .split(size);

    header::draw_header(f, app, chunks[0]);
    process_table::draw_process_table(f, app, chunks[1]);
    footer::draw_footer(f, app, chunks[2]);

    // Overlay popups
    match app.mode {
        AppMode::Help => help::draw_help(f),
        AppMode::SortSelect => sort_menu::draw_sort_menu(f, app),
        AppMode::Kill => kill_menu::draw_kill_menu(f, app),
        AppMode::UserFilter => user_menu::draw_user_menu(f, app),
        AppMode::Affinity => affinity_menu::draw_affinity_menu(f, app),
        _ => {}
    }
}
