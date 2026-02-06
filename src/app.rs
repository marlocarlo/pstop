use std::collections::{HashMap, HashSet};

use crate::system::cpu::CpuInfo;
use crate::system::memory::MemoryInfo;
use crate::system::process::{ProcessInfo, ProcessSortField};

/// Which view/mode the app is currently in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Search,      // F3: incremental search — jumps cursor to match, no filtering
    Filter,      // F4: incremental filter — hides non-matching processes
    Help,
    SortSelect,
    Kill,
    UserFilter,
}

/// Main application state
pub struct App {
    pub mode: AppMode,
    pub should_quit: bool,
    pub paused: bool,       // Z key: freeze/pause updates

    // System data
    pub cpu_info: CpuInfo,
    pub memory_info: MemoryInfo,
    pub processes: Vec<ProcessInfo>,
    pub filtered_processes: Vec<ProcessInfo>,

    // Process table state
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub visible_rows: usize,

    // Sorting
    pub sort_field: ProcessSortField,
    pub sort_ascending: bool,
    pub sort_menu_index: usize,

    // Search (F3) — transient, doesn't filter
    pub search_query: String,

    // Filter (F4) — persistent filter, hides non-matches
    pub filter_query: String,

    // User filter
    pub user_filter: Option<String>,
    pub available_users: Vec<String>,
    pub user_menu_index: usize,

    // Process tagging
    pub tagged_pids: HashSet<u32>,

    // Follow process
    pub follow_pid: Option<u32>,

    // Tree view
    pub tree_view: bool,
    /// Collapsed PIDs in tree view (collapsed subtree roots)
    pub collapsed_pids: HashSet<u32>,

    // Show threads
    pub show_threads: bool,

    // Show full paths to commands (htop 'p' toggle)
    pub show_full_path: bool,

    // Uptime & tasks
    pub uptime_seconds: u64,
    pub total_tasks: usize,
    pub running_tasks: usize,
    pub sleeping_tasks: usize,
    pub total_threads: usize,

    // Load average (approximated on Windows via CPU queue)
    pub load_avg_1: f64,
    pub load_avg_5: f64,
    pub load_avg_15: f64,

    // Kill mode signal selection
    pub kill_signal_index: usize,

    // Tick counter for refresh
    pub tick: u64,
}

/// Windows "signals" for kill menu (mapped to taskkill behavior)
pub const KILL_SIGNALS: &[(&str, &str)] = &[
    ("15", "SIGTERM   (graceful)"),
    ("9",  "SIGKILL   (force)"),
    ("1",  "SIGHUP    (hangup)"),
    ("2",  "SIGINT    (interrupt)"),
    ("3",  "SIGQUIT   (quit)"),
];

impl App {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Normal,
            should_quit: false,
            paused: false,

            cpu_info: CpuInfo::default(),
            memory_info: MemoryInfo::default(),
            processes: Vec::new(),
            filtered_processes: Vec::new(),

            selected_index: 0,
            scroll_offset: 0,
            visible_rows: 20,

            sort_field: ProcessSortField::Cpu,
            sort_ascending: false,
            sort_menu_index: 8, // CPU% index in all()

            search_query: String::new(),
            filter_query: String::new(),

            user_filter: None,
            available_users: Vec::new(),
            user_menu_index: 0,

            tagged_pids: HashSet::new(),
            follow_pid: None,

            tree_view: false,
            collapsed_pids: HashSet::new(),
            show_threads: false,
            show_full_path: false,

            uptime_seconds: 0,
            total_tasks: 0,
            running_tasks: 0,
            sleeping_tasks: 0,
            total_threads: 0,

            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,

            kill_signal_index: 1, // Default to SIGKILL (force) on Windows

            tick: 0,
        }
    }

    /// Apply sorting to the process list
    pub fn sort_processes(&mut self) {
        let ascending = self.sort_ascending;
        let field = self.sort_field;

        self.filtered_processes.sort_by(|a, b| {
            let ord = match field {
                ProcessSortField::Pid => a.pid.cmp(&b.pid),
                ProcessSortField::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                ProcessSortField::Priority => a.priority.cmp(&b.priority),
                ProcessSortField::Nice => a.nice.cmp(&b.nice),
                ProcessSortField::VirtMem => a.virtual_mem.cmp(&b.virtual_mem),
                ProcessSortField::ResMem => a.resident_mem.cmp(&b.resident_mem),
                ProcessSortField::SharedMem => a.shared_mem.cmp(&b.shared_mem),
                ProcessSortField::Cpu => a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortField::Mem => a.mem_usage.partial_cmp(&b.mem_usage).unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortField::Time => a.run_time.cmp(&b.run_time),
                ProcessSortField::Command => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                ProcessSortField::Status => a.status.cmp(&b.status),
            };
            if ascending { ord } else { ord.reverse() }
        });
    }

    /// Apply user filter and F4 filter query to process list
    pub fn apply_filter(&mut self) {
        self.filtered_processes = self.processes.clone();

        // User filter
        if let Some(ref user) = self.user_filter {
            let u = user.to_lowercase();
            self.filtered_processes.retain(|p| p.user.to_lowercase() == u);
        }

        // F4 persistent filter (filter_query)
        if !self.filter_query.is_empty() {
            let query = self.filter_query.to_lowercase();
            self.filtered_processes.retain(|p| {
                p.name.to_lowercase().contains(&query)
                    || p.command.to_lowercase().contains(&query)
                    || p.pid.to_string().contains(&query)
                    || p.user.to_lowercase().contains(&query)
            });
        }
    }

    /// F3 search: find next process matching search_query and jump to it
    pub fn search_next(&mut self) {
        if self.search_query.is_empty() || self.filtered_processes.is_empty() {
            return;
        }
        let query = self.search_query.to_lowercase();
        let start = self.selected_index + 1;
        let len = self.filtered_processes.len();

        // Search forward from current position, wrapping around
        for offset in 0..len {
            let idx = (start + offset) % len;
            let p = &self.filtered_processes[idx];
            if p.name.to_lowercase().contains(&query)
                || p.command.to_lowercase().contains(&query)
                || p.pid.to_string().contains(&query)
            {
                self.selected_index = idx;
                self.ensure_visible();
                return;
            }
        }
    }

    /// F3 search: find first match from top (when query changes)
    pub fn search_first(&mut self) {
        if self.search_query.is_empty() || self.filtered_processes.is_empty() {
            return;
        }
        let query = self.search_query.to_lowercase();
        for (idx, p) in self.filtered_processes.iter().enumerate() {
            if p.name.to_lowercase().contains(&query)
                || p.command.to_lowercase().contains(&query)
                || p.pid.to_string().contains(&query)
            {
                self.selected_index = idx;
                self.ensure_visible();
                return;
            }
        }
    }

    /// Ensure selected_index is visible in the viewport
    fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = self.selected_index - self.visible_rows + 1;
        }
    }

    /// Collect unique usernames from current process list
    pub fn collect_users(&mut self) {
        let mut users: Vec<String> = self.processes
            .iter()
            .map(|p| p.user.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        users.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        self.available_users = users;
    }

    /// Build tree view by organizing processes by parent-child relationship
    pub fn build_tree_view(&mut self) {
        let mut children_map: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut root_indices: Vec<usize> = Vec::new();

        for (i, proc) in self.filtered_processes.iter().enumerate() {
            if proc.ppid == 0 || !self.filtered_processes.iter().any(|p| p.pid == proc.ppid) {
                root_indices.push(i);
            } else {
                children_map.entry(proc.ppid).or_default().push(i);
            }
        }

        let mut ordered: Vec<(usize, usize, bool)> = Vec::new();

        fn dfs(
            idx: usize,
            depth: usize,
            is_last: bool,
            processes: &[ProcessInfo],
            children_map: &HashMap<u32, Vec<usize>>,
            collapsed: &HashSet<u32>,
            ordered: &mut Vec<(usize, usize, bool)>,
        ) {
            ordered.push((idx, depth, is_last));
            let pid = processes[idx].pid;
            // If this subtree is collapsed, don't recurse into children
            if collapsed.contains(&pid) {
                return;
            }
            if let Some(children) = children_map.get(&pid) {
                let len = children.len();
                for (ci, &child_idx) in children.iter().enumerate() {
                    dfs(child_idx, depth + 1, ci == len - 1, processes, children_map, collapsed, ordered);
                }
            }
        }

        let len = root_indices.len();
        for (ri, &root_idx) in root_indices.iter().enumerate() {
            dfs(root_idx, 0, ri == len - 1, &self.filtered_processes, &children_map, &self.collapsed_pids, &mut ordered);
        }

        let old_procs = self.filtered_processes.clone();
        self.filtered_processes.clear();
        for (idx, depth, is_last) in ordered {
            let mut proc = old_procs[idx].clone();
            proc.depth = depth;
            proc.is_last_child = is_last;
            self.filtered_processes.push(proc);
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let max = if self.filtered_processes.is_empty() {
            0
        } else {
            self.filtered_processes.len() - 1
        };
        if self.selected_index < max {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + self.visible_rows {
                self.scroll_offset = self.selected_index - self.visible_rows + 1;
            }
        }
    }

    /// Page up
    pub fn page_up(&mut self) {
        if self.selected_index > self.visible_rows {
            self.selected_index -= self.visible_rows;
        } else {
            self.selected_index = 0;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Page down
    pub fn page_down(&mut self) {
        let max = if self.filtered_processes.is_empty() {
            0
        } else {
            self.filtered_processes.len() - 1
        };
        self.selected_index = (self.selected_index + self.visible_rows).min(max);
        if self.selected_index >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = self.selected_index - self.visible_rows + 1;
        }
    }

    /// Home
    pub fn select_first(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// End
    pub fn select_last(&mut self) {
        if !self.filtered_processes.is_empty() {
            self.selected_index = self.filtered_processes.len() - 1;
            if self.selected_index >= self.visible_rows {
                self.scroll_offset = self.selected_index - self.visible_rows + 1;
            }
        }
    }

    /// Get the currently selected process
    pub fn selected_process(&self) -> Option<&ProcessInfo> {
        self.filtered_processes.get(self.selected_index)
    }

    /// Toggle sort field (cycle through or set specific)
    pub fn set_sort_field(&mut self, field: ProcessSortField) {
        if self.sort_field == field {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_field = field;
            self.sort_ascending = false;
        }
    }

    /// Toggle tag on selected process
    pub fn toggle_tag_selected(&mut self) {
        if let Some(proc) = self.selected_process() {
            let pid = proc.pid;
            if self.tagged_pids.contains(&pid) {
                self.tagged_pids.remove(&pid);
            } else {
                self.tagged_pids.insert(pid);
            }
        }
    }

    /// Tag selected process and all its children (htop 'c')
    pub fn tag_with_children(&mut self) {
        if let Some(proc) = self.selected_process() {
            let root_pid = proc.pid;
            // Collect all descendants
            let mut to_tag = vec![root_pid];
            let mut i = 0;
            while i < to_tag.len() {
                let parent = to_tag[i];
                for p in &self.filtered_processes {
                    if p.ppid == parent && !to_tag.contains(&p.pid) {
                        to_tag.push(p.pid);
                    }
                }
                i += 1;
            }
            for pid in to_tag {
                self.tagged_pids.insert(pid);
            }
        }
    }

    /// Follow selected process
    pub fn toggle_follow(&mut self) {
        if let Some(proc) = self.selected_process() {
            if self.follow_pid == Some(proc.pid) {
                self.follow_pid = None;
            } else {
                self.follow_pid = Some(proc.pid);
            }
        }
    }

    /// If following a process, keep it selected after sort/filter
    pub fn follow_process(&mut self) {
        if let Some(follow) = self.follow_pid {
            if let Some(idx) = self.filtered_processes.iter().position(|p| p.pid == follow) {
                self.selected_index = idx;
                self.ensure_visible();
            }
        }
    }

    /// Clamp selection to valid range
    pub fn clamp_selection(&mut self) {
        if self.filtered_processes.is_empty() {
            self.selected_index = 0;
            self.scroll_offset = 0;
        } else if self.selected_index >= self.filtered_processes.len() {
            self.selected_index = self.filtered_processes.len() - 1;
        }
    }
}
