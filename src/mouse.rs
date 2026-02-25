use crossterm::event::{MouseEvent, MouseEventKind, MouseButton};

use crate::app::{App, AppMode, ProcessTab};
use crate::system::process::ProcessSortField;
use crate::ui;
use crate::ui::process_table::{HEADERS, IO_HEADERS, NET_HEADERS, compute_display_columns};

/// Handle a mouse event.
/// Requires the terminal size (columns, rows) to compute layout areas.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent, term_width: u16, term_height: u16) {
    let h_height = ui::header_height(app);

    // Layout zones (same as ui::draw):
    //   [0]  y: 0          .. h_height-1          => header
    //   [1]  y: h_height                           => tab bar  (1 row)
    //   [2]  y: h_height+1 .. term_height-2        => process table
    //            first row of [2] = column header
    //            remaining rows   = process data
    //   [3]  y: term_height-1                      => footer (F-key bar)
    let tab_bar_y = h_height;
    let proc_start_y = h_height + 1; // process table area start
    let footer_y = term_height.saturating_sub(1);
    let header_row_y = proc_start_y; // column header is the first row of the process area
    let data_start_y = proc_start_y + 1; // data rows start after column header
    // data rows end just before footer
    let data_end_y = footer_y; // exclusive

    let x = mouse.column;
    let y = mouse.row;

    match mouse.kind {
        MouseEventKind::ScrollUp => app.select_prev(),
        MouseEventKind::ScrollDown => app.select_next(),

        MouseEventKind::Down(MouseButton::Left) => {
            // Only handle clicks in Normal mode (overlays handle their own input)
            if app.mode != AppMode::Normal {
                return;
            }

            if y == tab_bar_y {
                handle_tab_bar_click(app, x);
            } else if y == header_row_y {
                handle_header_click(app, x, term_width);
            } else if y >= data_start_y && y < data_end_y {
                handle_row_click(app, y, data_start_y);
            } else if y == footer_y {
                handle_footer_click(app, x);
            }
        }

        _ => {}
    }
}

// ── Tab bar click ────────────────────────────────────────────────────

/// Tab bar layout: " " (1) + " Main " (6) + " " (1) + " I/O " (5) + " " (1) + " Net " (5)
/// Main: x in [1..7), I/O: x in [8..13), Net: x in [14..19)
fn handle_tab_bar_click(app: &mut App, x: u16) {
    if (1..7).contains(&x) {
        app.active_tab = ProcessTab::Main;
    } else if (8..13).contains(&x) {
        app.active_tab = ProcessTab::Io;
    } else if (14..19).contains(&x) {
        app.active_tab = ProcessTab::Net;
    }
}

// ── Header click (sort by column) ───────────────────────────────────

fn handle_header_click(app: &mut App, x: u16, term_width: u16) {
    let headers = match app.active_tab {
        ProcessTab::Main => HEADERS,
        ProcessTab::Io   => IO_HEADERS,
        ProcessTab::Net  => NET_HEADERS,
    };

    // Compute display columns (same logic as rendering, so clicks match)
    let base_visible: std::collections::HashSet<ProcessSortField> = match app.active_tab {
        ProcessTab::Main => app.visible_columns.clone(),
        _ => headers.iter().map(|(_, _, f, _)| *f).collect(),
    };
    let display_cols = compute_display_columns(headers, &base_visible, term_width, app.sort_field);

    // Compute column boundaries, respecting auto-hidden columns
    let mut cursor: u16 = 0;
    for &(_, width, field, _) in headers {
        // Skip columns not in the display set
        if !display_cols.contains(&field) {
            continue;
        }

        let col_w = if width == 0 {
            // Command column: takes remaining space
            term_width.saturating_sub(cursor)
        } else {
            width
        };

        if x >= cursor && x < cursor + col_w {
            // Toggle sort direction if clicking same column, else switch
            app.set_sort_field(field);
            return;
        }

        cursor += col_w;
    }
}

// ── Process row click ───────────────────────────────────────────────

fn handle_row_click(app: &mut App, y: u16, data_start_y: u16) {
    let row_offset = (y - data_start_y) as usize;
    let target_index = app.scroll_offset + row_offset;

    if target_index < app.filtered_processes.len() {
        app.selected_index = target_index;
    }
}

// ── Footer (F-key bar) click ────────────────────────────────────────

/// F-key labels rendered in footer: "F1Help  " "F2Setup " etc.
/// Each entry is (key_label + desc), rendered sequentially.
const FKEYS_NORMAL: &[(&str, &str, FkeyAction)] = &[
    ("F1",  "Help  ",  FkeyAction::Help),
    ("F2",  "Setup ",  FkeyAction::Setup),
    ("F3",  "Search",  FkeyAction::Search),
    ("F4",  "Filter",  FkeyAction::Filter),
    ("F5",  "Tree  ",  FkeyAction::Tree),
    ("F6",  "SortBy",  FkeyAction::SortBy),
    ("F7",  "Nice -",  FkeyAction::NiceMinus),
    ("F8",  "Nice +",  FkeyAction::NicePlus),
    ("F9",  "Kill  ",  FkeyAction::Kill),
    ("F10", "Quit ",   FkeyAction::Quit),
];

#[derive(Clone, Copy)]
enum FkeyAction {
    Help,
    Setup,
    Search,
    Filter,
    Tree,
    SortBy,
    NiceMinus,
    NicePlus,
    Kill,
    Quit,
}

fn handle_footer_click(app: &mut App, x: u16) {
    let mut cursor: u16 = 0;

    for &(key_label, desc, action) in FKEYS_NORMAL {
        let entry_width = key_label.len() as u16 + desc.len() as u16;
        if x >= cursor && x < cursor + entry_width {
            execute_fkey_action(app, action);
            return;
        }
        cursor += entry_width;
    }
}

fn execute_fkey_action(app: &mut App, action: FkeyAction) {
    use crate::system::winapi;

    match action {
        FkeyAction::Help => {
            app.mode = AppMode::Help;
        }
        FkeyAction::Setup => {
            app.setup_menu_index = 0;
            app.mode = AppMode::Setup;
        }
        FkeyAction::Search => {
            app.mode = AppMode::Search;
            app.search_query.clear();
        }
        FkeyAction::Filter => {
            app.mode = AppMode::Filter;
        }
        FkeyAction::Tree => {
            app.tree_view = !app.tree_view;
            if app.tree_view {
                app.build_tree_view();
            }
        }
        FkeyAction::SortBy => {
            app.sort_menu_index = app.sort_field.index();
            app.mode = AppMode::SortSelect;
        }
        FkeyAction::NiceMinus => {
            if let Some(proc) = app.selected_process() {
                let _ok = winapi::raise_priority(proc.pid);
            }
        }
        FkeyAction::NicePlus => {
            if let Some(proc) = app.selected_process() {
                let _ok = winapi::lower_priority(proc.pid);
            }
        }
        FkeyAction::Kill => {
            app.mode = AppMode::Kill;
        }
        FkeyAction::Quit => {
            app.should_quit = true;
        }
    }
}
