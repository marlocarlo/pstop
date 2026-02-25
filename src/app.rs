use std::collections::{HashMap, HashSet};

use crate::color_scheme::{ColorScheme, ColorSchemeId};
use crate::system::cpu::CpuInfo;
use crate::system::gpu::GpuProcessInfo;
use crate::system::memory::MemoryInfo;
use crate::system::netstat::NetConnection;
use crate::system::network::NetworkInfo;
use crate::system::process::{ProcessInfo, ProcessSortField};

/// Which tab is active (htop Tab key switches between these)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTab {
    Main,  // Standard process view
    Io,    // I/O-focused view
    Net,   // Network connections view (real per-process connections)
    Gpu,   // GPU usage per process (GPU-agnostic via PDH)
}

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
    Affinity,    // a: CPU affinity selector
    Environment, // e: show process details/environment
    Setup,       // F2: setup menu (column/display configuration)
    Handles,     // l: list open files/handles (lsof equivalent)
}

/// Main application state
pub struct App {
    pub mode: AppMode,
    pub active_tab: ProcessTab, // Tab key switches between Main and I/O
    pub should_quit: bool,
    pub paused: bool,       // Z key: freeze/pause updates

    // Current user for shadow_other_users
    pub current_user: String,

    // System data
    pub cpu_info: CpuInfo,
    pub memory_info: MemoryInfo,
    pub network_info: NetworkInfo,
    pub processes: Vec<ProcessInfo>,
    pub filtered_processes: Vec<ProcessInfo>,

    // Network connections (Net tab)
    pub connections: Vec<NetConnection>,
    pub net_selected_index: usize,
    pub net_scroll_offset: usize,

    // GPU per-process data (GPU tab)
    pub gpu_processes: Vec<GpuProcessInfo>,
    pub gpu_adapter_name: String,
    pub gpu_overall_usage: f64,
    pub gpu_dedicated_mem: u64,     // Total dedicated GPU memory in use
    pub gpu_shared_mem: u64,        // Total shared GPU memory in use
    pub gpu_selected_index: usize,
    pub gpu_scroll_offset: usize,

    // Process table state
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub visible_rows: usize,

    // Sorting
    pub sort_field: ProcessSortField,
    pub sort_ascending: bool,
    pub sort_menu_index: usize,
    pub sort_scroll_offset: usize,

    // Search (F3) — transient, doesn't filter
    pub search_query: String,
    pub search_not_found: bool,

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

    // Hide kernel/system threads (htop 'K')
    pub hide_kernel_threads: bool,

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

    // CPU user/kernel time split (from GetSystemTimes)
    pub cpu_user_frac: f64,    // fraction of CPU time in user mode (0.0 - 1.0)
    pub cpu_kernel_frac: f64,  // fraction of CPU time in kernel mode (0.0 - 1.0)

    // Kill mode signal selection
    pub kill_signal_index: usize,

    // CPU affinity mode
    pub affinity_cpus: Vec<bool>, // CPU selection state (true = enabled)

    // Column visibility (F2 Setup menu)
    pub visible_columns: std::collections::HashSet<ProcessSortField>,
    pub setup_menu_index: usize,
    pub setup_category: usize,      // 0=Meters, 1=Display, 2=Colors, 3=Columns
    pub setup_panel: usize,         // 0=categories, 1=options/columns
    pub setup_meter_col: usize,     // 0=left, 1=right (Meters category)
    pub left_meters: Vec<String>,   // Configurable left header meters
    pub right_meters: Vec<String>,  // Configurable right header meters

    // Display options (F2 Setup → Display options) — full htop parity
    pub show_tree_by_default: bool,
    pub highlight_base_name: bool,
    pub shadow_other_users: bool,
    pub show_merged_command: bool,
    pub highlight_megabytes: bool,      // Highlight large memory values
    pub highlight_threads: bool,        // Display threads in different color
    pub header_margin: bool,            // Leave margin around header
    pub detailed_cpu_time: bool,        // Detailed CPU time breakdown
    pub cpu_count_from_zero: bool,      // Number CPUs from 0
    pub update_process_names: bool,     // Refresh process names each cycle
    pub show_thread_names: bool,        // Show custom thread names
    pub enable_mouse: bool,             // Mouse support on/off
    pub update_interval_ms: u64,        // Configurable refresh rate

    // Color scheme
    pub color_scheme_id: ColorSchemeId,
    pub color_scheme: ColorScheme,

    // Tick counter for refresh
    pub tick: u64,

    // Compact mode: minimal header for small screens/mobile
    pub compact_mode: bool,
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
            active_tab: ProcessTab::Main,
            should_quit: false,
            paused: false,

            current_user: std::env::var("USERNAME").unwrap_or_default().to_lowercase(),

            cpu_info: CpuInfo::default(),
            memory_info: MemoryInfo::default(),
            network_info: NetworkInfo::default(),
            processes: Vec::new(),
            filtered_processes: Vec::new(),

            connections: Vec::new(),
            net_selected_index: 0,
            net_scroll_offset: 0,

            gpu_processes: Vec::new(),
            gpu_adapter_name: String::new(),
            gpu_overall_usage: 0.0,
            gpu_dedicated_mem: 0,
            gpu_shared_mem: 0,
            gpu_selected_index: 0,
            gpu_scroll_offset: 0,

            selected_index: 0,
            scroll_offset: 0,
            visible_rows: 20,

            sort_field: ProcessSortField::Cpu,
            sort_ascending: false,
            sort_menu_index: 9,
            sort_scroll_offset: 0,

            search_query: String::new(),
            search_not_found: false,
            filter_query: String::new(),

            user_filter: None,
            available_users: Vec::new(),
            user_menu_index: 0,

            tagged_pids: HashSet::new(),
            follow_pid: None,

            tree_view: false,
            collapsed_pids: HashSet::new(),
            show_threads: false,
            hide_kernel_threads: false,
            show_full_path: false,

            uptime_seconds: 0,
            total_tasks: 0,
            running_tasks: 0,
            sleeping_tasks: 0,
            total_threads: 0,

            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,

            cpu_user_frac: 0.7,
            cpu_kernel_frac: 0.3,

            kill_signal_index: 1, // Default to SIGKILL (force) on Windows

            affinity_cpus: Vec::new(),

            // Default visible columns (htop default set)
            visible_columns: [
                ProcessSortField::Pid,
                ProcessSortField::User,
                ProcessSortField::Priority,
                ProcessSortField::Nice,
                ProcessSortField::VirtMem,
                ProcessSortField::ResMem,
                ProcessSortField::SharedMem,
                ProcessSortField::Status,
                ProcessSortField::Cpu,
                ProcessSortField::Mem,
                ProcessSortField::Time,
                ProcessSortField::Command,
            ].iter().cloned().collect(),
            setup_menu_index: 0,
            setup_category: 0,
            setup_panel: 0,
            setup_meter_col: 0,
            left_meters: vec![
                "CPUs (1/1)".to_string(),
                "Memory".to_string(),
                "Swap".to_string(),
                "Network".to_string(),
            ],
            right_meters: vec![
                "CPUs (2/2)".to_string(),
                "Tasks".to_string(),
                "Load average".to_string(),
                "Uptime".to_string(),
            ],
            show_tree_by_default: false,
            highlight_base_name: true,
            shadow_other_users: false,
            show_merged_command: false,
            highlight_megabytes: true,
            highlight_threads: true,
            header_margin: true,
            detailed_cpu_time: false,
            cpu_count_from_zero: false,
            update_process_names: false,
            show_thread_names: false,
            enable_mouse: true,
            update_interval_ms: 1500,

            color_scheme_id: ColorSchemeId::Default,
            color_scheme: ColorScheme::from_id(ColorSchemeId::Default),

            tick: 0,

            compact_mode: false,
        }
    }

    /// Apply sorting to the process list
    pub fn sort_processes(&mut self) {
        let ascending = self.sort_ascending;
        let field = self.sort_field;

        self.filtered_processes.sort_by(|a, b| {
            let ord = match field {
                ProcessSortField::Pid => a.pid.cmp(&b.pid),
                ProcessSortField::Ppid => a.ppid.cmp(&b.ppid),
                ProcessSortField::User => a.user.to_lowercase().cmp(&b.user.to_lowercase()),
                ProcessSortField::Priority => a.priority.cmp(&b.priority),
                ProcessSortField::Nice => a.nice.cmp(&b.nice),
                ProcessSortField::VirtMem => a.virtual_mem.cmp(&b.virtual_mem),
                ProcessSortField::ResMem => a.resident_mem.cmp(&b.resident_mem),
                ProcessSortField::SharedMem => a.shared_mem.cmp(&b.shared_mem),
                ProcessSortField::Cpu => a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortField::Mem => a.mem_usage.partial_cmp(&b.mem_usage).unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortField::Time => a.run_time.cmp(&b.run_time),
                ProcessSortField::Threads => a.threads.cmp(&b.threads),
                ProcessSortField::Command => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                ProcessSortField::Status => a.status.cmp(&b.status),
                ProcessSortField::IoReadRate => a.io_read_rate.partial_cmp(&b.io_read_rate).unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortField::IoWriteRate => a.io_write_rate.partial_cmp(&b.io_write_rate).unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortField::IoRate => {
                    let a_total = a.io_read_rate + a.io_write_rate;
                    let b_total = b.io_read_rate + b.io_write_rate;
                    a_total.partial_cmp(&b_total).unwrap_or(std::cmp::Ordering::Equal)
                }
            };
            if ascending { ord } else { ord.reverse() }
        });
    }

    /// Apply user filter and F4 filter query to process list
    pub fn apply_filter(&mut self) {
        // Build filtered list from processes — filters inline to avoid full clone
        let user_filter = self.user_filter.as_ref().map(|u| u.to_lowercase());
        let hide_kernel = self.hide_kernel_threads;
        let filter_empty = self.filter_query.is_empty();
        let query_lower = self.filter_query.to_lowercase();
        let terms: Vec<&str> = if !filter_empty { query_lower.split('|').collect() } else { vec![] };

        self.filtered_processes.clear();
        for p in &self.processes {
            // User filter
            if let Some(ref u) = user_filter {
                if p.user.to_lowercase() != *u {
                    continue;
                }
            }

            // Hide kernel threads (K toggle)
            if hide_kernel {
                let user_lower = p.user.to_lowercase();
                if user_lower.contains("system") || user_lower.contains("nt authority") {
                    continue;
                }
            }

            // F4 persistent filter
            if !filter_empty {
                let name_lower = p.name.to_lowercase();
                let cmd_lower = p.command.to_lowercase();
                let matches = terms.iter().any(|term| {
                    let t = term.trim();
                    if t.is_empty() { return false; }
                    name_lower.contains(t) || cmd_lower.contains(t)
                });
                if !matches {
                    continue;
                }
            }

            self.filtered_processes.push(p.clone());
        }
    }

    /// F3 search: find next process matching search_query and jump to it
    /// htop: searches Command column only, case-insensitive, substring match
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
            {
                self.selected_index = idx;
                self.search_not_found = false;
                self.ensure_visible();
                return;
            }
        }
        self.search_not_found = true;
    }

    /// Shift+F3 search: find previous process matching search_query
    /// htop: Shift+F3 cycles backwards through matches
    pub fn search_prev(&mut self) {
        if self.search_query.is_empty() || self.filtered_processes.is_empty() {
            return;
        }
        let query = self.search_query.to_lowercase();
        let len = self.filtered_processes.len();
        let start = if self.selected_index == 0 { len - 1 } else { self.selected_index - 1 };

        // Search backward from current position, wrapping around
        for offset in 0..len {
            let idx = (start + len - offset) % len;
            let p = &self.filtered_processes[idx];
            if p.name.to_lowercase().contains(&query)
                || p.command.to_lowercase().contains(&query)
            {
                self.selected_index = idx;
                self.search_not_found = false;
                self.ensure_visible();
                return;
            }
        }
        self.search_not_found = true;
    }

    /// F3 search: find first match from top (when query changes)
    /// htop: incremental search jumps to first match as you type
    pub fn search_first(&mut self) {
        if self.search_query.is_empty() || self.filtered_processes.is_empty() {
            self.search_not_found = false;
            return;
        }
        let query = self.search_query.to_lowercase();
        for (idx, p) in self.filtered_processes.iter().enumerate() {
            if p.name.to_lowercase().contains(&query)
                || p.command.to_lowercase().contains(&query)
            {
                self.selected_index = idx;
                self.search_not_found = false;
                self.ensure_visible();
                return;
            }
        }
        self.search_not_found = true;
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
        // Build a HashSet of all PIDs for O(1) parent lookup (was O(n²))
        let all_pids: HashSet<u32> = self.filtered_processes.iter().map(|p| p.pid).collect();

        let mut children_map: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut root_indices: Vec<usize> = Vec::new();

        for (i, proc) in self.filtered_processes.iter().enumerate() {
            if proc.ppid == 0 || !all_pids.contains(&proc.ppid) {
                root_indices.push(i);
            } else {
                children_map.entry(proc.ppid).or_default().push(i);
            }
        }

        let mut ordered: Vec<(usize, usize, bool)> = Vec::with_capacity(self.filtered_processes.len());

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

        // Rebuild in-place: collect into a new vec, then swap
        let mut new_procs = Vec::with_capacity(ordered.len());
        for (idx, depth, is_last) in ordered {
            let mut proc = self.filtered_processes[idx].clone();
            proc.depth = depth;
            proc.is_last_child = is_last;
            new_procs.push(proc);
        }
        self.filtered_processes = new_procs;
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        let idx = self.active_selected_index_mut();
        if *idx > 0 {
            *idx -= 1;
            let idx_val = *idx;
            let scroll = self.active_scroll_offset_mut();
            if idx_val < *scroll {
                *scroll = idx_val;
            }
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let max = self.active_list_len().saturating_sub(1);
        let idx = self.active_selected_index_mut();
        if *idx < max {
            *idx += 1;
            let idx_val = *idx;
            let visible = self.visible_rows;
            let scroll = self.active_scroll_offset_mut();
            if idx_val >= *scroll + visible {
                *scroll = idx_val - visible + 1;
            }
        }
    }

    /// Page up
    pub fn page_up(&mut self) {
        let visible = self.visible_rows;
        let idx = self.active_selected_index_mut();
        if *idx > visible {
            *idx -= visible;
        } else {
            *idx = 0;
        }
        let idx_val = *idx;
        let scroll = self.active_scroll_offset_mut();
        if idx_val < *scroll {
            *scroll = idx_val;
        }
    }

    /// Page down
    pub fn page_down(&mut self) {
        let max = self.active_list_len().saturating_sub(1);
        let visible = self.visible_rows;
        let idx = self.active_selected_index_mut();
        *idx = (*idx + visible).min(max);
        let idx_val = *idx;
        let scroll = self.active_scroll_offset_mut();
        if idx_val >= *scroll + visible {
            *scroll = idx_val - visible + 1;
        }
    }

    /// Home
    pub fn select_first(&mut self) {
        *self.active_selected_index_mut() = 0;
        *self.active_scroll_offset_mut() = 0;
    }

    /// End
    pub fn select_last(&mut self) {
        let len = self.active_list_len();
        if len > 0 {
            let visible = self.visible_rows;
            let last = len - 1;
            *self.active_selected_index_mut() = last;
            let scroll = self.active_scroll_offset_mut();
            if last >= visible {
                *scroll = last - visible + 1;
            }
        }
    }

    /// Get the active list length for the current tab
    fn active_list_len(&self) -> usize {
        match self.active_tab {
            ProcessTab::Main | ProcessTab::Io => self.filtered_processes.len(),
            ProcessTab::Net => self.connections.len(),
            ProcessTab::Gpu => self.gpu_processes.len(),
        }
    }

    /// Get mutable ref to the selected index for the current tab
    fn active_selected_index_mut(&mut self) -> &mut usize {
        match self.active_tab {
            ProcessTab::Main | ProcessTab::Io => &mut self.selected_index,
            ProcessTab::Net => &mut self.net_selected_index,
            ProcessTab::Gpu => &mut self.gpu_selected_index,
        }
    }

    /// Get mutable ref to the scroll offset for the current tab
    fn active_scroll_offset_mut(&mut self) -> &mut usize {
        match self.active_tab {
            ProcessTab::Main | ProcessTab::Io => &mut self.scroll_offset,
            ProcessTab::Net => &mut self.net_scroll_offset,
            ProcessTab::Gpu => &mut self.gpu_scroll_offset,
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
            // Build parent→children map for O(n) traversal
            let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
            for p in &self.filtered_processes {
                children_map.entry(p.ppid).or_default().push(p.pid);
            }
            // BFS from root
            let mut to_tag = vec![root_pid];
            let mut visited = HashSet::new();
            visited.insert(root_pid);
            let mut i = 0;
            while i < to_tag.len() {
                let parent = to_tag[i];
                if let Some(children) = children_map.get(&parent) {
                    for &child in children {
                        if visited.insert(child) {
                            to_tag.push(child);
                        }
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
        // Clamp process list selections (Main/Io)
        if self.filtered_processes.is_empty() {
            self.selected_index = 0;
            self.scroll_offset = 0;
        } else if self.selected_index >= self.filtered_processes.len() {
            self.selected_index = self.filtered_processes.len() - 1;
        }
        // Clamp Net tab selection
        if self.connections.is_empty() {
            self.net_selected_index = 0;
            self.net_scroll_offset = 0;
        } else if self.net_selected_index >= self.connections.len() {
            self.net_selected_index = self.connections.len() - 1;
        }
        // Clamp GPU tab selection
        if self.gpu_processes.is_empty() {
            self.gpu_selected_index = 0;
            self.gpu_scroll_offset = 0;
        } else if self.gpu_selected_index >= self.gpu_processes.len() {
            self.gpu_selected_index = self.gpu_processes.len() - 1;
        }
    }
}
