use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, ProcessTab, KILL_SIGNALS};
use crate::system::process::ProcessSortField;
use crate::system::winapi;

/// Handle a single key input event.
pub fn handle_input(app: &mut App, key: KeyEvent) {
    match app.mode {
        AppMode::Normal    => handle_normal_mode(app, key),
        AppMode::Search    => handle_search_mode(app, key),
        AppMode::Filter    => handle_filter_mode(app, key),
        AppMode::Help      => handle_help_mode(app, key),
        AppMode::SortSelect => handle_sort_mode(app, key),
        AppMode::Kill      => handle_kill_mode(app, key),
        AppMode::UserFilter => handle_user_filter_mode(app, key),
        AppMode::Affinity  => handle_affinity_mode(app, key),
        AppMode::Environment => handle_environment_mode(app, key),
        AppMode::Setup     => handle_setup_mode(app, key),
        AppMode::Handles   => handle_handles_mode(app, key),
    }
}

// ── Normal mode ─────────────────────────────────────────────────────────

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        // ── Quit ──
        KeyCode::F(10) | KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }

        // ── Navigation (arrows + Alt-j/Alt-k per htop man page) ──
        KeyCode::Up    => app.select_prev(),
        KeyCode::Down  => app.select_next(),
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => app.select_prev(),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => app.select_next(),
        KeyCode::PageUp  => app.page_up(),
        KeyCode::PageDown => app.page_down(),
        KeyCode::Home  => app.select_first(),
        KeyCode::End   => app.select_last(),

        // ── Tab key: switch between Main, I/O, and Net tabs ──
        KeyCode::Tab => {
            app.active_tab = match app.active_tab {
                ProcessTab::Main => ProcessTab::Io,
                ProcessTab::Io => ProcessTab::Net,
                ProcessTab::Net => ProcessTab::Main,
            };
        }
        KeyCode::BackTab => {
            // Shift+Tab goes backwards
            app.active_tab = match app.active_tab {
                ProcessTab::Main => ProcessTab::Net,
                ProcessTab::Io => ProcessTab::Main,
                ProcessTab::Net => ProcessTab::Io,
            };
        }

        // ── Help ──
        KeyCode::F(1) | KeyCode::Char('?') => app.mode = AppMode::Help,
        KeyCode::Char('h') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.mode = AppMode::Help;
        }

        // ── F2 / Setup menu — configure columns and display ──
        KeyCode::F(2) | KeyCode::Char('S') => {
            app.setup_menu_index = 0;
            app.mode = AppMode::Setup;
        }

        // ── F3 / Search — jump to match, no filtering ──
        KeyCode::F(3) | KeyCode::Char('/') => {
            app.mode = AppMode::Search;
            app.search_query.clear();
        }

        // ── F4 / \ — persistent filter, hides non-matching ──
        KeyCode::F(4) | KeyCode::Char('\\') => {
            app.mode = AppMode::Filter;
            // Don't clear filter_query — let user edit the existing filter
        }

        // ── F5 / t — toggle tree view ──
        KeyCode::F(5) | KeyCode::Char('t') => {
            app.tree_view = !app.tree_view;
            if app.tree_view {
                app.build_tree_view();
            }
        }

        // ── F6 — sort menu ──
        KeyCode::F(6) => {
            app.sort_menu_index = app.sort_field.index();
            app.sort_scroll_offset = 0;
            // Ensure current selection is visible
            if app.sort_menu_index >= 10 {
                app.sort_scroll_offset = app.sort_menu_index.saturating_sub(9);
            }
            app.mode = AppMode::SortSelect;
        }

        // ── Sort shortcuts ──
        KeyCode::Char('<') | KeyCode::Char(',') => cycle_sort_field(app, false),
        KeyCode::Char('>') | KeyCode::Char('.') => cycle_sort_field(app, true),
        KeyCode::Char('P') => app.set_sort_field(ProcessSortField::Cpu),
        KeyCode::Char('M') => app.set_sort_field(ProcessSortField::Mem),
        KeyCode::Char('T') => app.set_sort_field(ProcessSortField::Time),
        KeyCode::Char('N') => app.set_sort_field(ProcessSortField::Pid),
        KeyCode::Char('I') => app.sort_ascending = !app.sort_ascending,

        // ── F7 — Nice - (raise priority / lower nice) ──
        KeyCode::F(7) => {
            if let Some(proc) = app.selected_process() {
                let _ok = winapi::raise_priority(proc.pid);
            }
        }

        // ── F8 — Nice + (lower priority / raise nice) ──
        KeyCode::F(8) => {
            if let Some(proc) = app.selected_process() {
                let _ok = winapi::lower_priority(proc.pid);
            }
        }

        // ── F9 / k — kill (htop: k = kill) ──
        KeyCode::F(9) | KeyCode::Char('k') => {
            app.mode = AppMode::Kill;
        }

        // ── User filter (htop 'u') ──
        KeyCode::Char('u') => {
            app.user_menu_index = 0;
            app.mode = AppMode::UserFilter;
        }

        // ── Follow process (htop 'F') ──
        KeyCode::Char('F') => app.toggle_follow(),

        // ── Tag process (htop Space) — tag and move down ──
        KeyCode::Char(' ') => {
            app.toggle_tag_selected();
            app.select_next();
        }

        // ── Untag all (htop 'U') ──
        KeyCode::Char('U') => app.tagged_pids.clear(),

        // ── Tag process + children (htop 'c') ──
        KeyCode::Char('c') => app.tag_with_children(),

        // ── Toggle show threads (htop 'H') ──
        KeyCode::Char('H') => app.show_threads = !app.show_threads,

        // ── Toggle hide kernel/system threads (htop 'K') ──
        KeyCode::Char('K') => app.hide_kernel_threads = !app.hide_kernel_threads,

        // ── Pause/freeze updates (htop 'Z') ──
        KeyCode::Char('Z') | KeyCode::Char('z') => app.paused = !app.paused,

        // ── Ctrl-L — force full refresh ──
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.paused = false; // unpause if paused
            // refresh will happen on next tick
        }

        // ── Tree expand/collapse (+/-/*) ──
        KeyCode::Char('+') | KeyCode::Char('=') => {
            if app.tree_view {
                let pid = app.selected_process().map(|p| p.pid);
                if let Some(pid) = pid {
                    app.collapsed_pids.remove(&pid);
                    app.build_tree_view();
                }
            }
        }
        KeyCode::Char('-') => {
            if app.tree_view {
                let pid = app.selected_process().map(|p| p.pid);
                if let Some(pid) = pid {
                    app.collapsed_pids.insert(pid);
                    app.build_tree_view();
                }
            }
        }
        KeyCode::Char('*') => {
            // Expand all collapsed subtrees
            if app.tree_view {
                app.collapsed_pids.clear();
                app.build_tree_view();
            }
        }

        // ── Toggle full path display (htop 'p') ──
        KeyCode::Char('p') => app.show_full_path = !app.show_full_path,

        // ── CPU affinity (htop 'a') ──
        KeyCode::Char('a') => {
            if let Some(proc) = app.selected_process() {
                let cpu_count = winapi::get_cpu_count();
                let (proc_mask, _sys_mask, success) = winapi::get_process_affinity(proc.pid);
                if success {
                    // Initialize affinity_cpus based on current mask
                    app.affinity_cpus = (0..cpu_count)
                        .map(|i| (proc_mask & (1 << i)) != 0)
                        .collect();
                    app.mode = AppMode::Affinity;
                }
            }
        }

        // ── Show process environment/details (htop 'e') ──
        KeyCode::Char('e') => {
            if app.selected_process().is_some() {
                app.mode = AppMode::Environment;
            }
        }

        // ── List open files/handles (htop 'l' - lsof equivalent) ──
        KeyCode::Char('l') => {
            if app.selected_process().is_some() {
                app.mode = AppMode::Handles;
            }
        }

        // ── Number keys: quick PID search ──
        KeyCode::Char(c) if c.is_ascii_digit() => {
            // Switch to search mode with the digit pre-filled
            app.mode = AppMode::Search;
            app.search_query.clear();
            app.search_query.push(c);
            app.search_first();
        }

        _ => {}
    }
}

// ── F3 Search mode: jump to match, don't filter ─────────────────────────

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.search_query.clear();
            app.search_not_found = false;
        }
        KeyCode::Enter => {
            // Find next match (htop behavior)
            app.search_next();
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.search_first();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.search_first();
        }
        KeyCode::Up   => app.select_prev(),
        KeyCode::Down  => app.select_next(),
        KeyCode::F(10) => app.should_quit = true,
        KeyCode::F(3) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+F3 = find previous (htop behavior)
                app.search_prev();
            } else {
                // F3 again = find next
                app.search_next();
            }
        }
        _ => {}
    }
}

// ── F4 Filter mode: hide non-matching processes ─────────────────────────

fn handle_filter_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.filter_query.clear();
            app.apply_filter();
            app.sort_processes();
            if app.tree_view { app.build_tree_view(); }
            app.clamp_selection();
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            // Confirm filter and return to normal mode (filter stays active)
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.filter_query.pop();
            app.apply_filter();
            app.sort_processes();
            if app.tree_view { app.build_tree_view(); }
            app.clamp_selection();
        }
        KeyCode::Char(c) => {
            app.filter_query.push(c);
            app.apply_filter();
            app.sort_processes();
            if app.tree_view { app.build_tree_view(); }
            app.clamp_selection();
        }
        KeyCode::Up   => app.select_prev(),
        KeyCode::Down  => app.select_next(),
        KeyCode::F(10) => app.should_quit = true,
        _ => {}
    }
}

// ── Help mode ───────────────────────────────────────────────────────────

fn handle_help_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('q') | KeyCode::Enter => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Sort selection mode — arrow-key navigable ───────────────────────────

fn handle_sort_mode(app: &mut App, key: KeyEvent) {
    let field_count = ProcessSortField::all().len();
    // Estimate visible items in sort menu (70% of terminal, minus borders/hints)
    // visible_rows approximates process area height; terminal is roughly visible_rows + header + footer + extras
    let approx_terminal_h = app.visible_rows + 10;
    let sort_menu_h = (approx_terminal_h * 70 / 100).max(8);
    let sort_visible = sort_menu_h.saturating_sub(4); // borders (2) + blank + hint line

    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Up => {
            if app.sort_menu_index > 0 {
                app.sort_menu_index -= 1;
                if app.sort_menu_index < app.sort_scroll_offset {
                    app.sort_scroll_offset = app.sort_menu_index;
                }
            }
        }
        KeyCode::Down => {
            if app.sort_menu_index + 1 < field_count {
                app.sort_menu_index += 1;
                // Scroll down if cursor goes past visible area
                if app.sort_menu_index >= app.sort_scroll_offset + sort_visible {
                    app.sort_scroll_offset = app.sort_menu_index - sort_visible + 1;
                }
            }
        }
        KeyCode::Home => {
            app.sort_menu_index = 0;
            app.sort_scroll_offset = 0;
        }
        KeyCode::End => {
            app.sort_menu_index = field_count.saturating_sub(1);
            if app.sort_menu_index >= sort_visible {
                app.sort_scroll_offset = app.sort_menu_index - sort_visible + 1;
            }
        }
        KeyCode::Enter => {
            let fields = ProcessSortField::all();
            if app.sort_menu_index < fields.len() {
                app.set_sort_field(fields[app.sort_menu_index]);
            }
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Kill mode — signal selection ────────────────────────────────────────

fn handle_kill_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Up => {
            if app.kill_signal_index > 0 {
                app.kill_signal_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.kill_signal_index + 1 < KILL_SIGNALS.len() {
                app.kill_signal_index += 1;
            }
        }
        KeyCode::Enter => {
            let pids: Vec<u32> = if !app.tagged_pids.is_empty() {
                app.tagged_pids.iter().copied().collect()
            } else if let Some(proc) = app.selected_process() {
                vec![proc.pid]
            } else {
                vec![]
            };

            for pid in pids {
                kill_process_with_signal(pid, app.kill_signal_index);
            }
            app.tagged_pids.clear();
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── User filter mode — pick a user from the list ────────────────────────

fn handle_user_filter_mode(app: &mut App, key: KeyEvent) {
    let max_idx = app.available_users.len(); // 0 = "All users", 1..N = actual users
    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Up => {
            if app.user_menu_index > 0 {
                app.user_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.user_menu_index < max_idx {
                app.user_menu_index += 1;
            }
        }
        KeyCode::Enter => {
            if app.user_menu_index == 0 {
                app.user_filter = None;
            } else {
                let user_idx = app.user_menu_index - 1;
                if user_idx < app.available_users.len() {
                    app.user_filter = Some(app.available_users[user_idx].clone());
                }
            }
            app.apply_filter();
            app.sort_processes();
            if app.tree_view { app.build_tree_view(); }
            app.clamp_selection();
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── CPU Affinity mode ───────────────────────────────────────────────────

fn handle_affinity_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            // Apply the affinity mask
            if let Some(proc) = app.selected_process() {
                let mut mask: usize = 0;
                for (i, &enabled) in app.affinity_cpus.iter().enumerate() {
                    if enabled {
                        mask |= 1 << i;
                    }
                }
                if mask != 0 {
                    let _ = winapi::set_process_affinity(proc.pid, mask);
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char(' ') => {
            // Space: toggle CPU 0
            if !app.affinity_cpus.is_empty() {
                app.affinity_cpus[0] = !app.affinity_cpus[0];
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            // Number key: toggle specific CPU
            if let Some(cpu_idx) = c.to_digit(10) {
                let idx = cpu_idx as usize;
                if idx < app.affinity_cpus.len() {
                    app.affinity_cpus[idx] = !app.affinity_cpus[idx];
                }
            }
        }
        KeyCode::Char('a') => {
            // Toggle all CPUs
            let all_on = app.affinity_cpus.iter().all(|&x| x);
            for cpu in &mut app.affinity_cpus {
                *cpu = !all_on;
            }
        }
        _ => {}
    }
}

// ── Environment/Details mode ────────────────────────────────────────────

fn handle_environment_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('e') | KeyCode::Char('q') | KeyCode::Enter => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Handles view mode (l - lsof) ────────────────────────────────────────

fn handle_handles_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('l') | KeyCode::Char('q') | KeyCode::Enter => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Setup/Configuration mode (F2) ───────────────────────────────────────

fn handle_setup_mode(app: &mut App, key: KeyEvent) {
    use crate::color_scheme::{ColorScheme, ColorSchemeId};
    let all_fields = ProcessSortField::all();
    let num_categories = 4usize; // Meters, Display options, Colors, Columns
    // Max index in content panel per category
    let max_content_idx = match app.setup_category {
        0 => 3,  // 4 meters per column
        1 => 14, // 14 display options + interval row
        2 => ColorSchemeId::all().len().saturating_sub(1),
        3 => all_fields.len().saturating_sub(1), // All fields, not just visible ones
        _ => 0,
    };

    match key.code {
        KeyCode::Esc | KeyCode::F(2) | KeyCode::F(10) => {
            // Save config when exiting setup
            let _ = crate::config::PstopConfig::from_app(app).save();
            app.mode = AppMode::Normal;
        }
        // ── Panel switching ──
        KeyCode::Left => {
            if app.setup_panel > 0 {
                app.setup_panel -= 1;
                app.setup_menu_index = 0;
            }
        }
        KeyCode::Right => {
            if app.setup_panel < 1 {
                app.setup_panel += 1;
                app.setup_menu_index = 0;
            }
        }
        // ── Navigation ──
        KeyCode::Up => {
            if app.setup_panel == 0 {
                if app.setup_category > 0 {
                    app.setup_category -= 1;
                    app.setup_menu_index = 0;
                }
            } else if app.setup_menu_index > 0 {
                app.setup_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.setup_panel == 0 {
                if app.setup_category + 1 < num_categories {
                    app.setup_category += 1;
                    app.setup_menu_index = 0;
                }
            } else if app.setup_menu_index < max_content_idx {
                app.setup_menu_index += 1;
            }
        }
        // ── Actions ──
        KeyCode::Char(' ') | KeyCode::Enter => {
            if app.setup_panel == 0 {
                app.setup_panel = 1;
                app.setup_menu_index = 0;
            } else {
                match app.setup_category {
                    1 => {
                        // Display options toggles (14 options + interval)
                        match app.setup_menu_index {
                            0  => app.show_tree_by_default = !app.show_tree_by_default,
                            1  => app.shadow_other_users = !app.shadow_other_users,
                            2  => app.hide_kernel_threads = !app.hide_kernel_threads,
                            3  => app.highlight_base_name = !app.highlight_base_name,
                            4  => app.highlight_megabytes = !app.highlight_megabytes,
                            5  => app.highlight_threads = !app.highlight_threads,
                            6  => app.header_margin = !app.header_margin,
                            7  => app.detailed_cpu_time = !app.detailed_cpu_time,
                            8  => app.cpu_count_from_zero = !app.cpu_count_from_zero,
                            9  => app.update_process_names = !app.update_process_names,
                            10 => app.show_thread_names = !app.show_thread_names,
                            11 => app.show_full_path = !app.show_full_path,
                            12 => app.show_merged_command = !app.show_merged_command,
                            13 => app.enable_mouse = !app.enable_mouse,
                            _ => {} // interval row, use +/-
                        }
                    }
                    2 => {
                        // Apply color scheme
                        let new_id = ColorSchemeId::from_index(app.setup_menu_index);
                        app.color_scheme_id = new_id;
                        app.color_scheme = ColorScheme::from_id(new_id);
                    }
                    3 => {
                        // Toggle column visibility (add or remove)
                        if let Some(&field) = all_fields.get(app.setup_menu_index) {
                            if field != ProcessSortField::Command {
                                // Command is always visible
                                if app.visible_columns.contains(&field) {
                                    app.visible_columns.remove(&field);
                                } else {
                                    app.visible_columns.insert(field);
                                }
                            }
                        }
                    }
                    _ => {} // Meters: future
                }
            }
        }
        KeyCode::Char('a') => {
            // Toggle all columns (Columns category only)
            if app.setup_category == 3 && app.setup_panel == 1 {
                if app.visible_columns.len() == all_fields.len() {
                    app.visible_columns.clear();
                    app.visible_columns.insert(ProcessSortField::Command);
                } else {
                    for field in all_fields {
                        app.visible_columns.insert(*field);
                    }
                }
                app.setup_menu_index = 0;
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            if app.setup_category == 1 {
                app.update_interval_ms = (app.update_interval_ms + 100).min(10000);
            }
        }
        KeyCode::Char('-') => {
            if app.setup_category == 1 {
                app.update_interval_ms = app.update_interval_ms.saturating_sub(100).max(200);
            }
        }
        _ => {}
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Kill a process by PID on Windows using taskkill
/// signal_index: 0=SIGTERM (graceful), 1=SIGKILL (force), etc.
fn kill_process_with_signal(pid: u32, signal_index: usize) {
    use std::process::Command;
    match signal_index {
        0 => {
            // SIGTERM equivalent: try graceful close via taskkill without /F
            let result = Command::new("taskkill")
                .args(["/PID", &pid.to_string()])
                .output();
            // If graceful fails, don't force — user chose graceful
            let _ = result;
        }
        _ => {
            // SIGKILL and others: force kill
            let _ = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
        }
    }
}

/// Cycle through sort fields
fn cycle_sort_field(app: &mut App, forward: bool) {
    let fields = ProcessSortField::all();
    let current_idx = fields.iter().position(|f| *f == app.sort_field).unwrap_or(0);
    let new_idx = if forward {
        (current_idx + 1) % fields.len()
    } else {
        if current_idx == 0 { fields.len() - 1 } else { current_idx - 1 }
    };
    app.sort_field = fields[new_idx];
}
