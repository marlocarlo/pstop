//! pstop configuration persistence (htoprc-style key=value format)
//!
//! Saves/loads settings to `%APPDATA%/pstop/pstoprc` on Windows.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::color_scheme::{ColorScheme, ColorSchemeId};
use crate::system::process::ProcessSortField;

/// Get the config file path: %APPDATA%/pstop/pstoprc
fn config_path() -> Option<PathBuf> {
    std::env::var("APPDATA").ok().map(|appdata| {
        PathBuf::from(appdata).join("pstop").join("pstoprc")
    })
}

/// Persistable settings (subset of App state)
pub struct PstopConfig {
    // Display options
    pub tree_view: bool,
    pub show_tree_by_default: bool,
    pub hide_kernel_threads: bool,
    pub shadow_other_users: bool,
    pub highlight_base_name: bool,
    pub show_full_path: bool,
    pub show_merged_command: bool,
    pub highlight_megabytes: bool,
    pub highlight_threads: bool,
    pub header_margin: bool,
    pub detailed_cpu_time: bool,
    pub cpu_count_from_zero: bool,
    pub update_process_names: bool,
    pub show_thread_names: bool,
    pub enable_mouse: bool,
    pub update_interval_ms: u64,

    // Color scheme
    pub color_scheme_id: ColorSchemeId,

    // Sorting
    pub sort_field: ProcessSortField,
    pub sort_ascending: bool,

    // Visible columns
    pub visible_columns: Vec<ProcessSortField>,
}

impl Default for PstopConfig {
    fn default() -> Self {
        Self {
            tree_view: false,
            show_tree_by_default: false,
            hide_kernel_threads: false,
            shadow_other_users: false,
            highlight_base_name: true,
            show_full_path: false,
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
            sort_field: ProcessSortField::Cpu,
            sort_ascending: false,
            visible_columns: ProcessSortField::all().to_vec(),
        }
    }
}

impl PstopConfig {
    /// Load config from file, returning defaults if file doesn't exist
    pub fn load() -> Self {
        let path = match config_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        let mut cfg = Self::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "tree_view" => cfg.tree_view = value == "1",
                    "show_tree_by_default" => cfg.show_tree_by_default = value == "1",
                    "hide_kernel_threads" => cfg.hide_kernel_threads = value == "1",
                    "shadow_other_users" => cfg.shadow_other_users = value == "1",
                    "highlight_base_name" => cfg.highlight_base_name = value == "1",
                    "show_full_path" => cfg.show_full_path = value == "1",
                    "show_merged_command" => cfg.show_merged_command = value == "1",
                    "highlight_megabytes" => cfg.highlight_megabytes = value == "1",
                    "highlight_threads" => cfg.highlight_threads = value == "1",
                    "header_margin" => cfg.header_margin = value == "1",
                    "detailed_cpu_time" => cfg.detailed_cpu_time = value == "1",
                    "cpu_count_from_zero" => cfg.cpu_count_from_zero = value == "1",
                    "update_process_names" => cfg.update_process_names = value == "1",
                    "show_thread_names" => cfg.show_thread_names = value == "1",
                    "enable_mouse" => cfg.enable_mouse = value == "1",
                    "update_interval_ms" => {
                        if let Ok(v) = value.parse::<u64>() {
                            cfg.update_interval_ms = v.max(200).min(10000);
                        }
                    }
                    "color_scheme" => {
                        if let Ok(idx) = value.parse::<usize>() {
                            cfg.color_scheme_id = ColorSchemeId::from_index(idx);
                        }
                    }
                    "sort_field" => {
                        if let Ok(idx) = value.parse::<usize>() {
                            let all = ProcessSortField::all();
                            if idx < all.len() {
                                cfg.sort_field = all[idx];
                            }
                        }
                    }
                    "sort_ascending" => cfg.sort_ascending = value == "1",
                    "visible_columns" => {
                        let all = ProcessSortField::all();
                        let indices: Vec<usize> = value.split(',')
                            .filter_map(|s| s.trim().parse::<usize>().ok())
                            .collect();
                        if !indices.is_empty() {
                            cfg.visible_columns = indices.iter()
                                .filter(|&&i| i < all.len())
                                .map(|&i| all[i])
                                .collect();
                        }
                    }
                    _ => {} // Ignore unknown keys
                }
            }
        }

        cfg
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), String> {
        let path = match config_path() {
            Some(p) => p,
            None => return Err("Could not determine config path".into()),
        };

        // Create parent directory
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let mut lines = Vec::new();
        lines.push("# pstop configuration file".to_string());
        lines.push(format!("# Auto-generated — do not edit while pstop is running"));
        lines.push(String::new());

        let b = |v: bool| if v { "1" } else { "0" };

        lines.push(format!("tree_view={}", b(self.tree_view)));
        lines.push(format!("show_tree_by_default={}", b(self.show_tree_by_default)));
        lines.push(format!("hide_kernel_threads={}", b(self.hide_kernel_threads)));
        lines.push(format!("shadow_other_users={}", b(self.shadow_other_users)));
        lines.push(format!("highlight_base_name={}", b(self.highlight_base_name)));
        lines.push(format!("show_full_path={}", b(self.show_full_path)));
        lines.push(format!("show_merged_command={}", b(self.show_merged_command)));
        lines.push(format!("highlight_megabytes={}", b(self.highlight_megabytes)));
        lines.push(format!("highlight_threads={}", b(self.highlight_threads)));
        lines.push(format!("header_margin={}", b(self.header_margin)));
        lines.push(format!("detailed_cpu_time={}", b(self.detailed_cpu_time)));
        lines.push(format!("cpu_count_from_zero={}", b(self.cpu_count_from_zero)));
        lines.push(format!("update_process_names={}", b(self.update_process_names)));
        lines.push(format!("show_thread_names={}", b(self.show_thread_names)));
        lines.push(format!("enable_mouse={}", b(self.enable_mouse)));
        lines.push(format!("update_interval_ms={}", self.update_interval_ms));
        lines.push(format!("color_scheme={}", self.color_scheme_id as usize));
        
        // Sort field index
        let all_fields = ProcessSortField::all();
        let sort_idx = all_fields.iter().position(|f| *f == self.sort_field).unwrap_or(0);
        lines.push(format!("sort_field={}", sort_idx));
        lines.push(format!("sort_ascending={}", b(self.sort_ascending)));

        // Visible columns as comma-separated indices
        let col_indices: Vec<String> = self.visible_columns.iter()
            .filter_map(|col| all_fields.iter().position(|f| f == col))
            .map(|i| i.to_string())
            .collect();
        lines.push(format!("visible_columns={}", col_indices.join(",")));

        let content = lines.join("\n") + "\n";
        let mut file = fs::File::create(&path)
            .map_err(|e| format!("Failed to create config file: {}", e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write config file: {}", e))?;

        Ok(())
    }

    /// Build config from current App state
    pub fn from_app(app: &crate::app::App) -> Self {
        Self {
            tree_view: app.tree_view,
            show_tree_by_default: app.show_tree_by_default,
            hide_kernel_threads: app.hide_kernel_threads,
            shadow_other_users: app.shadow_other_users,
            highlight_base_name: app.highlight_base_name,
            show_full_path: app.show_full_path,
            show_merged_command: app.show_merged_command,
            highlight_megabytes: app.highlight_megabytes,
            highlight_threads: app.highlight_threads,
            header_margin: app.header_margin,
            detailed_cpu_time: app.detailed_cpu_time,
            cpu_count_from_zero: app.cpu_count_from_zero,
            update_process_names: app.update_process_names,
            show_thread_names: app.show_thread_names,
            enable_mouse: app.enable_mouse,
            update_interval_ms: app.update_interval_ms,
            color_scheme_id: app.color_scheme_id,
            sort_field: app.sort_field,
            sort_ascending: app.sort_ascending,
            visible_columns: app.visible_columns.iter().cloned().collect(),
        }
    }

    /// Apply loaded config to App state
    pub fn apply_to(&self, app: &mut crate::app::App) {
        app.tree_view = self.tree_view;
        app.show_tree_by_default = self.show_tree_by_default;
        app.hide_kernel_threads = self.hide_kernel_threads;
        app.shadow_other_users = self.shadow_other_users;
        app.highlight_base_name = self.highlight_base_name;
        app.show_full_path = self.show_full_path;
        app.show_merged_command = self.show_merged_command;
        app.highlight_megabytes = self.highlight_megabytes;
        app.highlight_threads = self.highlight_threads;
        app.header_margin = self.header_margin;
        app.detailed_cpu_time = self.detailed_cpu_time;
        app.cpu_count_from_zero = self.cpu_count_from_zero;
        app.update_process_names = self.update_process_names;
        app.show_thread_names = self.show_thread_names;
        app.enable_mouse = self.enable_mouse;
        app.update_interval_ms = self.update_interval_ms;
        app.color_scheme_id = self.color_scheme_id;
        app.color_scheme = ColorScheme::from_id(self.color_scheme_id);
        app.sort_field = self.sort_field;
        app.sort_ascending = self.sort_ascending;
        app.visible_columns = self.visible_columns.iter().cloned().collect();
    }
}
