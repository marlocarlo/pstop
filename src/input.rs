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

        // ── Tab key: switch between Main and I/O tabs (htop Tab) ──
        KeyCode::Tab => {
            app.active_tab = match app.active_tab {
                ProcessTab::Main => ProcessTab::Io,
                ProcessTab::Io => ProcessTab::Main,
            };
        }
        KeyCode::BackTab => {
            // Shift+Tab goes backwards
            app.active_tab = match app.active_tab {
                ProcessTab::Main => ProcessTab::Io,
                ProcessTab::Io => ProcessTab::Main,
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
        }
        KeyCode::Enter => {
            // Find next match
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
            // F3 again = find next
            app.search_next();
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
    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Up => {
            if app.sort_menu_index > 0 {
                app.sort_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.sort_menu_index + 1 < field_count {
                app.sort_menu_index += 1;
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
                kill_process(pid);
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
    let all_fields = ProcessSortField::all();
    
    match key.code {
        KeyCode::Esc | KeyCode::F(2) | KeyCode::F(10) => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Up => {
            if app.setup_menu_index > 0 {
                app.setup_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.setup_menu_index + 1 < all_fields.len() {
                app.setup_menu_index += 1;
            }
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            // Toggle column visibility
            let field = all_fields[app.setup_menu_index];
            // Don't allow hiding Command column (always needed)
            if field != ProcessSortField::Command {
                if app.visible_columns.contains(&field) {
                    app.visible_columns.remove(&field);
                } else {
                    app.visible_columns.insert(field);
                }
            }
        }
        KeyCode::Char('a') => {
            // Toggle all columns
            if app.visible_columns.len() == all_fields.len() {
                // If all visible, hide all except Command
                app.visible_columns.clear();
                app.visible_columns.insert(ProcessSortField::Command);
            } else {
                // Show all
                for field in all_fields {
                    app.visible_columns.insert(*field);
                }
            }
        }
        _ => {}
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Kill a process by PID on Windows using taskkill
fn kill_process(pid: u32) {
    use std::process::Command;
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
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
