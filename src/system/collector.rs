use std::collections::HashMap;

use sysinfo::{System, ProcessStatus as SysProcessStatus, ProcessesToUpdate, Networks};

use crate::app::App;
use crate::system::cpu::{CpuCore, CpuInfo};
use crate::system::gpu::GpuCollector;
use crate::system::memory::MemoryInfo;
use crate::system::network::NetworkInfo;
use crate::system::process::{ProcessInfo, ProcessStatus};
use crate::system::winapi;
use crate::system::netstat;

/// System data collector using the `sysinfo` crate, with Windows user resolution
pub struct Collector {
    sys: System,
    networks: Networks,
    /// Cache: PID -> resolved user name (via Win32 token lookup)
    user_name_cache: HashMap<u32, String>,
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
    /// Real boot time (Unix timestamp) from Windows Event Log.
    /// Accounts for Fast Startup, which causes GetTickCount64() to report
    /// inflated uptime because the kernel hibernates instead of rebooting.
    boot_time_unix: Option<i64>,
    /// CPU user/kernel time split tracker (via GetSystemTimes)
    cpu_time_split: winapi::CpuTimeSplit,
    /// Last sampled CPU user/kernel fractions
    pub cpu_user_frac: f64,
    pub cpu_kernel_frac: f64,
    /// GPU collector (persistent PDH query)
    gpu_collector: GpuCollector,
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

        let networks = Networks::new_with_refreshed_list();

        // Query real boot time from Event Log (handles Fast Startup correctly)
        let boot_time_unix = winapi::get_real_boot_time();

        Self {
            sys,
            networks,
            user_name_cache: HashMap::new(),
            win_data_cache: HashMap::new(),
            win_data_cache_ticks: 0,
            prev_io_counters: HashMap::new(),
            prev_net_rx: 0,
            prev_net_tx: 0,
            prev_net_time: None,
            load_samples_1: 0.0,
            load_samples_5: 0.0,
            load_samples_15: 0.0,
            boot_time_unix,
            cpu_time_split: winapi::CpuTimeSplit::new(),
            cpu_user_frac: 0.7,
            cpu_kernel_frac: 0.3,
            gpu_collector: GpuCollector::new(),
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
        // update_process_names: when false, skip re-fetching exe/name/command (expensive)
        self.sys.refresh_processes(ProcessesToUpdate::All, app.update_process_names);

        // Sample real CPU user/kernel split via GetSystemTimes
        let (user_frac, kernel_frac) = self.cpu_time_split.sample();
        self.cpu_user_frac = user_frac;
        self.cpu_kernel_frac = kernel_frac;

        self.collect_cpu(app);
        self.collect_memory(app);
        self.collect_network(app);
        self.collect_processes(app);
        self.collect_uptime(app);
        self.compute_load_average(app);

        // Pass CPU user/kernel split to app for header rendering
        app.cpu_user_frac = self.cpu_user_frac;
        app.cpu_kernel_frac = self.cpu_kernel_frac;

        app.collect_users();
        app.apply_filter();
        app.sort_processes();

        // Rebuild tree AFTER sorting if tree view is active
        if app.tree_view {
            app.build_tree_view();
        }

        // ── Network bandwidth (Net tab) ──
        // Only collect when on the Net tab (avoid overhead otherwise)
        if matches!(app.active_tab, crate::app::ProcessTab::Net) {
            let conn_counts = netstat::count_connections_per_pid();

            // Build ProcessNetBandwidth by matching connection PIDs to process I/O rates
            let mut net_procs: Vec<netstat::ProcessNetBandwidth> = conn_counts
                .into_iter()
                .map(|(pid, count)| {
                    let (name, recv, send) = app.processes.iter()
                        .find(|p| p.pid == pid)
                        .map(|p| (p.name.clone(), p.io_read_rate, p.io_write_rate))
                        .unwrap_or_else(|| {
                            let name = if pid == 4 { "System".to_string() } else { format!("PID:{}", pid) };
                            (name, 0.0, 0.0)
                        });
                    netstat::ProcessNetBandwidth {
                        pid,
                        name,
                        recv_bytes_per_sec: recv,
                        send_bytes_per_sec: send,
                        connection_count: count,
                    }
                })
                .collect();

            // Sort: highest total bandwidth first, then by connection count
            net_procs.sort_by(|a, b| {
                let ar = a.recv_bytes_per_sec + a.send_bytes_per_sec;
                let br = b.recv_bytes_per_sec + b.send_bytes_per_sec;
                br.partial_cmp(&ar)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.connection_count.cmp(&a.connection_count))
            });

            app.net_processes = net_procs;
        }

        // ── GPU per-process data (GPU tab) ──
        if matches!(app.active_tab, crate::app::ProcessTab::Gpu) {
            app.gpu_processes = self.gpu_collector.collect();
            // Sort by GPU usage descending so busiest processes are at top
            app.gpu_processes.sort_by(|a, b| b.gpu_usage.partial_cmp(&a.gpu_usage).unwrap_or(std::cmp::Ordering::Equal));

            // Populate overall GPU stats for header meters
            let info = &self.gpu_collector.adapter_info;
            app.gpu_overall_usage = info.overall_usage;
            app.gpu_dedicated_mem = info.total_dedicated_mem;
            app.gpu_shared_mem = info.total_shared_mem;
            if app.gpu_adapter_name.is_empty() {
                app.gpu_adapter_name = crate::system::gpu::detect_gpu_adapter_name();
            }
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
        let uptime = self.real_uptime();
        let mut running = 0usize;
        let mut sleeping = 0usize;
        let mut total_threads = 0usize;

        // Collect raw process data first (no &mut self needed)
        let raw_procs: Vec<(u32, u32, String, String, SysProcessStatus, u64, u64, f32, f32, u64)> = self.sys.processes()
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
                let name = proc_info.name().to_string_lossy().to_string();

                (pid.as_u32(), ppid, name, command, proc_info.status(), virt, resident, proc_info.cpu_usage(), mem_pct, proc_info.run_time())
            })
            .collect();

        // Batch-collect Windows-specific data (priority, thread counts)
        // Only refresh every 3 ticks to reduce expensive Win32 API overhead
        let all_pids: Vec<u32> = raw_procs.iter().map(|(pid, ..)| *pid).collect();
        if self.win_data_cache_ticks == 0 || self.win_data_cache_ticks % 3 == 0 {
            self.win_data_cache = winapi::collect_process_data(&all_pids);
            // Also refresh user names (same cadence — users don't change often)
            self.user_name_cache = winapi::batch_process_users(&all_pids);
        }
        self.win_data_cache_ticks += 1;

        // I/O counters MUST be fetched every tick for accurate rate calculation
        let io_counters = winapi::batch_io_counters(&all_pids);

        // Batch-collect per-process CPU times for TIME+ sub-second precision
        // Only every 3 ticks (aligned with win_data refresh) to save overhead
        let process_times = if self.win_data_cache_ticks % 3 == 1 {
            winapi::batch_process_times(&all_pids)
        } else {
            HashMap::new()
        };

        // Build a set of current PIDs for dead PID cleanup
        let current_pids: std::collections::HashSet<u32> = all_pids.iter().copied().collect();

        // Merge Win32 data into process list — access caches by reference, no cloning
        let processes: Vec<ProcessInfo> = raw_procs.into_iter()
            .map(|(pid, ppid, name, command, sys_status, virt, resident, cpu_usage, mem_pct, run_time)| {
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

                let user_name = self.user_name_cache.get(&pid).cloned().unwrap_or_else(|| "SYSTEM".to_string());

                // Get Win32 data (priority, nice, thread count)
                let wd = self.win_data_cache.get(&pid);
                let priority = wd.map(|d| d.priority).unwrap_or(8);
                let nice = wd.map(|d| d.nice).unwrap_or(0);
                let threads = wd.map(|d| d.thread_count).unwrap_or(1);
                let private_ws = wd.map(|d| d.private_working_set).unwrap_or(0);
                total_threads += threads as usize;

                // shared_mem = resident (working set) - private working set
                let shared_mem = resident.saturating_sub(private_ws);

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

                // Get high-precision CPU time for TIME+ display
                let cpu_time_100ns = process_times.get(&pid).copied().unwrap_or(0);

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
                    shared_mem,
                    cpu_usage,
                    mem_usage: mem_pct,
                    run_time: run_time.min(uptime),
                    cpu_time_100ns,
                    threads,
                    io_read_rate,
                    io_write_rate,
                    depth: 0,
                    is_last_child: false,
                }
            })
            .collect();

        // Clean up dead PIDs from prev_io_counters to prevent memory leak
        self.prev_io_counters.retain(|pid, _| current_pids.contains(pid));

        // If show_threads is enabled, enumerate individual threads and add as sub-entries
        if app.show_threads {
            let mut expanded = Vec::with_capacity(processes.len() * 2);
            for proc in processes {
                let pid = proc.pid;
                let threads_info = winapi::enumerate_threads(pid, app.show_thread_names);
                expanded.push(proc);
                for ti in threads_info {
                    let thread_name = if !ti.name.is_empty() {
                        ti.name
                    } else {
                        format!("tid:{}", ti.thread_id)
                    };
                    expanded.push(ProcessInfo {
                        pid: ti.thread_id,   // Use thread ID as PID for display
                        ppid: pid,           // Parent is the owning process
                        name: thread_name,
                        command: String::new(),
                        user: String::new(),
                        status: ProcessStatus::Running,
                        priority: ti.base_priority,
                        nice: 0,
                        virtual_mem: 0,
                        resident_mem: 0,
                        shared_mem: 0,
                        cpu_usage: 0.0,
                        mem_usage: 0.0,
                        run_time: 0,
                        cpu_time_100ns: 0,
                        threads: 0,
                        io_read_rate: 0.0,
                        io_write_rate: 0.0,
                        depth: 1,
                        is_last_child: false,
                    });
                }
            }
            app.processes = expanded;
        } else {
            app.processes = processes;
        }

        app.total_tasks = app.processes.len();
        app.running_tasks = running;
        app.sleeping_tasks = sleeping;
        app.total_threads = total_threads;
    }

    /// Calculate system uptime, using the real boot time from the Event Log
    /// when available (correctly handles Fast Startup), falling back to
    /// sysinfo's GetTickCount64-based uptime otherwise.
    fn real_uptime(&self) -> u64 {
        if let Some(boot_time) = self.boot_time_unix {
            let now = chrono::Utc::now().timestamp();
            (now - boot_time).max(0) as u64
        } else {
            System::uptime()
        }
    }

    fn collect_uptime(&self, app: &mut App) {
        app.uptime_seconds = self.real_uptime();
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
}
