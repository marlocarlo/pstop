use std::collections::HashMap;

use sysinfo::{System, ProcessStatus as SysProcessStatus, ProcessesToUpdate, Users, Networks};

use crate::app::App;
use crate::system::cpu::{CpuCore, CpuInfo};
use crate::system::memory::MemoryInfo;
use crate::system::network::NetworkInfo;
use crate::system::process::{ProcessInfo, ProcessStatus};
use crate::system::winapi;

/// System data collector using the `sysinfo` crate, with Windows user resolution
pub struct Collector {
    sys: System,
    users: Users,
    networks: Networks,
    /// Cache: sysinfo user_id string -> resolved display name
    user_cache: HashMap<String, String>,
    /// Cache: Win32 process data (priority, threads) - updated every 3 ticks
    win_data_cache: HashMap<u32, winapi::WinProcessData>,
    win_data_cache_ticks: u64,
    /// Previous I/O counters for rate calculation: PID -> (read_bytes, write_bytes, timestamp)
    prev_io_counters: HashMap<u32, (u64, u64, std::time::Instant)>,
    /// Previous network totals for rate calculation
    prev_net_rx: u64,
    prev_net_tx: u64,
    prev_net_time: Option<std::time::Instant>,
    /// Exponential moving averages for load approximation
    load_samples_1: f64,
    load_samples_5: f64,
    load_samples_15: f64,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new();
        // Only refresh what we need initially
        sys.refresh_cpu_all();
        sys.refresh_memory();
        
        // Need an initial CPU measurement for deltas
        std::thread::sleep(std::time::Duration::from_millis(100));
        sys.refresh_cpu_all();

        let users = Users::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        Self {
            sys,
            users,
            networks,
            user_cache: HashMap::new(),
            win_data_cache: HashMap::new(),
            win_data_cache_ticks: 0,
            prev_io_counters: HashMap::new(),
            prev_net_rx: 0,
            prev_net_tx: 0,
            prev_net_time: None,
            load_samples_1: 0.0,
            load_samples_5: 0.0,
            load_samples_15: 0.0,
        }
    }

    /// Refresh all system data and populate the App
    pub fn refresh(&mut self, app: &mut App) {
        if app.paused {
            return; // Z key: freeze display
        }

        // Refresh only what we need - much faster than refresh_all()
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        self.collect_cpu(app);
        self.collect_memory(app);
        self.collect_network(app);
        self.collect_processes(app);
        self.collect_uptime(app);
        self.compute_load_average(app);

        app.collect_users();
        app.apply_filter();
        app.sort_processes();

        // Rebuild tree AFTER sorting if tree view is active
        if app.tree_view {
            app.build_tree_view();
        }

        app.follow_process();
        app.clamp_selection();
        app.tick += 1;
    }

    fn collect_cpu(&self, app: &mut App) {
        let cpus = self.sys.cpus();

        let cores: Vec<CpuCore> = cpus
            .iter()
            .enumerate()
            .map(|(i, cpu)| CpuCore {
                id: i,
                usage_percent: cpu.cpu_usage(),
                frequency_mhz: cpu.frequency(),
            })
            .collect();

        let total_usage = if cores.is_empty() {
            0.0
        } else {
            cores.iter().map(|c| c.usage_percent).sum::<f32>() / cores.len() as f32
        };

        let brand = cpus.first().map(|c| c.brand().to_string()).unwrap_or_default();

        app.cpu_info = CpuInfo {
            physical_cores: self.sys.physical_core_count().unwrap_or(cores.len()),
            logical_cores: cores.len(),
            total_usage,
            brand,
            cores,
        };
    }

    fn collect_memory(&self, app: &mut App) {
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        let available = self.sys.available_memory();
        let free = total.saturating_sub(used);

        // Approximate cache = available - free (on Windows, "available" includes standby/cache)
        let cached = available.saturating_sub(free);

        app.memory_info = MemoryInfo {
            total_mem: total,
            used_mem: used,
            free_mem: free,
            cached_mem: cached,
            buffered_mem: 0, // Windows doesn't separate buffers
            total_swap: self.sys.total_swap(),
            used_swap: self.sys.used_swap(),
            free_swap: self.sys.free_swap(),
        };
    }

    fn collect_network(&mut self, app: &mut App) {
        // Refresh network data (true = reset delta counters)
        self.networks.refresh(true);

        let now = std::time::Instant::now();

        // Sum across all interfaces
        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;
        for (_name, data) in self.networks.iter() {
            total_rx += data.total_received();
            total_tx += data.total_transmitted();
        }

        let (rx_rate, tx_rate) = if let Some(prev_time) = self.prev_net_time {
            let elapsed = now.duration_since(prev_time).as_secs_f64();
            if elapsed > 0.0 {
                let rx = (total_rx.saturating_sub(self.prev_net_rx)) as f64 / elapsed;
                let tx = (total_tx.saturating_sub(self.prev_net_tx)) as f64 / elapsed;
                (rx, tx)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        self.prev_net_rx = total_rx;
        self.prev_net_tx = total_tx;
        self.prev_net_time = Some(now);

        app.network_info = NetworkInfo {
            rx_bytes_per_sec: rx_rate,
            tx_bytes_per_sec: tx_rate,
            total_rx,
            total_tx,
        };
    }

    fn collect_processes(&mut self, app: &mut App) {
        let total_mem = self.sys.total_memory();
        let uptime = System::uptime();
        let mut running = 0usize;
        let mut sleeping = 0usize;
        let mut total_threads = 0usize;

        // Collect raw process data first (no &mut self needed)
        let raw_procs: Vec<(u32, u32, String, String, Option<String>, SysProcessStatus, u64, u64, f32, f32, u64)> = self.sys.processes()
            .iter()
            .map(|(&pid, proc_info)| {
                let resident = proc_info.memory();
                let virt = proc_info.virtual_memory();
                let mem_pct = if total_mem > 0 {
                    (resident as f32 / total_mem as f32) * 100.0
                } else {
                    0.0
                };

                let cmd = proc_info.cmd();
                let command = if cmd.is_empty() {
                    proc_info.name().to_string_lossy().to_string()
                } else {
                    cmd.iter()
                        .map(|s| s.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                };

                let ppid = proc_info.parent().map(|p| p.as_u32()).unwrap_or(0);
                let uid_str = proc_info.user_id().map(|u| u.to_string());
                let name = proc_info.name().to_string_lossy().to_string();

                (pid.as_u32(), ppid, name, command, uid_str, proc_info.status(), virt, resident, proc_info.cpu_usage(), mem_pct, proc_info.run_time())
            })
            .collect();

        // Batch-collect Windows-specific data (priority, thread counts)
        // Only refresh every 3 ticks to reduce expensive Win32 API overhead
        let all_pids: Vec<u32> = raw_procs.iter().map(|(pid, ..)| *pid).collect();
        if self.win_data_cache_ticks == 0 || self.win_data_cache_ticks % 3 == 0 {
            self.win_data_cache = winapi::collect_process_data(&all_pids);
        }
        self.win_data_cache_ticks += 1;
        // Clone the cache to avoid borrow checker issues
        let win_data = self.win_data_cache.clone();

        // I/O counters MUST be fetched every tick for accurate rate calculation
        let io_counters = winapi::batch_io_counters(&all_pids);

        // Now resolve users (needs &mut self for cache) and merge Win32 data
        let processes: Vec<ProcessInfo> = raw_procs.into_iter()
            .map(|(pid, ppid, name, command, uid_str, sys_status, virt, resident, cpu_usage, mem_pct, run_time)| {
                let status = match sys_status {
                    SysProcessStatus::Run => {
                        running += 1;
                        ProcessStatus::Running
                    }
                    SysProcessStatus::Sleep => {
                        sleeping += 1;
                        ProcessStatus::Sleeping
                    }
                    SysProcessStatus::Stop => ProcessStatus::Stopped,
                    SysProcessStatus::Zombie => ProcessStatus::Zombie,
                    _ => {
                        sleeping += 1;
                        ProcessStatus::Sleeping
                    }
                };

                let user_name = self.resolve_user_by_uid(uid_str.as_deref());

                // Get Win32 data (priority, nice, thread count, I/O counters)
                let wd = win_data.get(&pid);
                let priority = wd.map(|d| d.priority).unwrap_or(8);
                let nice = wd.map(|d| d.nice).unwrap_or(0);
                let threads = wd.map(|d| d.thread_count).unwrap_or(1);
                total_threads += threads as usize;

                // Calculate I/O rates based on difference from previous tick
                let (io_read_bytes, io_write_bytes) = io_counters.get(&pid).copied().unwrap_or((0, 0));
                let now = std::time::Instant::now();
                
                let (io_read_rate, io_write_rate) = if let Some((prev_read, prev_write, prev_time)) = self.prev_io_counters.get(&pid) {
                    let elapsed = now.duration_since(*prev_time).as_secs_f64();
                    if elapsed > 0.0 {
                        let read_rate = (io_read_bytes.saturating_sub(*prev_read)) as f64 / elapsed;
                        let write_rate = (io_write_bytes.saturating_sub(*prev_write)) as f64 / elapsed;
                        (read_rate, write_rate)
                    } else {
                        (0.0, 0.0)
                    }
                } else {
                    (0.0, 0.0)
                };

                // Update prev counters for next tick
                self.prev_io_counters.insert(pid, (io_read_bytes, io_write_bytes, now));

                ProcessInfo {
                    pid,
                    ppid,
                    name,
                    command,
                    user: user_name,
                    status,
                    priority,
                    nice,
                    virtual_mem: virt,
                    resident_mem: resident,
                    shared_mem: 0, // Not easily available on Windows
                    cpu_usage,
                    mem_usage: mem_pct,
                    run_time: run_time.min(uptime),
                    threads,
                    io_read_rate,
                    io_write_rate,
                    depth: 0,
                    is_last_child: false,
                    tagged: false,
                }
            })
            .collect();

        app.total_tasks = processes.len();
        app.running_tasks = running;
        app.sleeping_tasks = sleeping;
        app.total_threads = total_threads;
        app.processes = processes;
    }

    fn collect_uptime(&self, app: &mut App) {
        app.uptime_seconds = System::uptime();
    }

    /// Approximate load averages using exponential moving average of CPU usage.
    /// Real load average doesn't exist on Windows, but this gives a useful approximation.
    fn compute_load_average(&mut self, app: &mut App) {
        let num_cores = app.cpu_info.cores.len().max(1) as f64;
        // Current "load" = fraction of cores busy
        let current_load = (app.cpu_info.total_usage as f64 / 100.0) * num_cores;

        // EMA constants for ~1s tick: alpha = 1 - e^(-interval/period)
        let alpha_1 = 1.0 - (-1.0_f64 / 60.0).exp();    // 1 min
        let alpha_5 = 1.0 - (-1.0_f64 / 300.0).exp();   // 5 min
        let alpha_15 = 1.0 - (-1.0_f64 / 900.0).exp();  // 15 min

        self.load_samples_1 += alpha_1 * (current_load - self.load_samples_1);
        self.load_samples_5 += alpha_5 * (current_load - self.load_samples_5);
        self.load_samples_15 += alpha_15 * (current_load - self.load_samples_15);

        app.load_avg_1 = self.load_samples_1;
        app.load_avg_5 = self.load_samples_5;
        app.load_avg_15 = self.load_samples_15;
    }

    /// Resolve user name from a uid string (already extracted from process)
    fn resolve_user_by_uid(&mut self, uid_str: Option<&str>) -> String {
        match uid_str {
            Some(uid) => {
                if let Some(cached) = self.user_cache.get(uid) {
                    return cached.clone();
                }
                // Search sysinfo Users list by string match
                let name = self.users.iter()
                    .find(|u| u.id().to_string() == uid)
                    .map(|u| u.name().to_string())
                    .unwrap_or_else(|| {
                        extract_short_sid(uid)
                    });
                self.user_cache.insert(uid.to_string(), name.clone());
                name
            }
            None => "SYSTEM".to_string(),
        }
    }
}

/// Extract a short name from a Windows SID string if user resolution fails
fn extract_short_sid(sid: &str) -> String {
    // Windows SIDs look like S-1-5-21-xxx-xxx-xxx-1001
    // Take last segment as a short identifier
    if let Some(last) = sid.rsplit('-').next() {
        match last {
            "18" => "SYSTEM".to_string(),
            "19" => "LOCAL SVC".to_string(),
            "20" => "NET SVC".to_string(),
            _ => format!("UID:{}", last),
        }
    } else {
        sid.to_string()
    }
}
