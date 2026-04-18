use std::collections::HashMap;

use sysinfo::{System, ProcessStatus as SysProcessStatus, ProcessesToUpdate, ProcessRefreshKind, UpdateKind, Networks};

use crate::app::App;
use crate::system::cpu::{CpuCore, CpuInfo};
use crate::system::gpu::GpuCollector;
use crate::system::memory::MemoryInfo;
use crate::system::network::NetworkInfo;
use crate::system::process::{ProcessInfo, ProcessStatus};
use crate::system::winapi;
use crate::system::netstat;

/// System data collector using the `sysinfo` crate for process data,
/// with native Win32 APIs for CPU monitoring and per-process enrichment.
pub struct Collector {
    pub(crate) sys: System,
    networks: Option<Networks>,
    /// Native per-core CPU monitor (replaces sysinfo PDH, saves ~155ms init)
    cpu_monitor: winapi::NativeCpuMonitor,
    /// Cache: PID -> resolved user name (via Win32 token lookup)
    user_name_cache: HashMap<u32, String>,
    /// Cache: Win32 process data (priority, threads) - updated every 3 ticks
    win_data_cache: HashMap<u32, winapi::WinProcessData>,
    win_data_cache_ticks: u64,
    /// Cache: per-process CPU times for TIME+ (updated every 3 ticks)
    process_times_cache: HashMap<u32, u64>,
    /// Previous I/O counters for rate calculation: PID -> (read_bytes, write_bytes, timestamp)
    prev_io_counters: HashMap<u32, (u64, u64, std::time::Instant)>,
    /// Pre-fetched I/O counters from parallel thread (consumed by collect_processes)
    prefetched_io: HashMap<u32, (u64, u64)>,
    /// Cache: PID -> (name, command) — used when update_process_names is OFF
    process_name_cache: HashMap<u32, (String, String)>,
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
    /// Pending boot time query (runs on background thread)
    boot_time_pending: Option<std::thread::JoinHandle<Option<i64>>>,
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
        // Spawn boot time query on background thread (it shells out to wevtutil ~200ms)
        // Don't block — we'll check for completion on first refresh
        let boot_time_handle = std::thread::spawn(|| winapi::get_real_boot_time());

        // Use native NtQuerySystemInformation for CPU monitoring (<1ms)
        // instead of sysinfo's PDH-based approach (~155ms initialization).
        let cpu_monitor = winapi::NativeCpuMonitor::new();

        let sys = System::new();
        // No refresh_cpu_all needed! CPU monitoring is handled by NativeCpuMonitor.
        // sysinfo is only used for process enumeration + memory.

        // Networks lazy-initialized on first use (saves ~12ms)

        Self {
            sys,
            networks: None,
            cpu_monitor,
            user_name_cache: HashMap::new(),
            win_data_cache: HashMap::new(),
            win_data_cache_ticks: 0,
            process_times_cache: HashMap::new(),
            prev_io_counters: HashMap::new(),
            prefetched_io: HashMap::new(),
            process_name_cache: HashMap::new(),
            prev_net_rx: 0,
            prev_net_tx: 0,
            prev_net_time: None,
            load_samples_1: 0.0,
            load_samples_5: 0.0,
            load_samples_15: 0.0,
            boot_time_unix: None,
            boot_time_pending: Some(boot_time_handle),
            cpu_time_split: winapi::CpuTimeSplit::new(),
            cpu_user_frac: 0.7,
            cpu_kernel_frac: 0.3,
            gpu_collector: GpuCollector::new(),
        }
    }

    /// Fast partial refresh: only CPU + memory for header bars.
    /// Used for the second frame so CPU/memory bars appear ~120ms before process data.
    pub fn refresh_header_only(&mut self, app: &mut App) {
        self.sys.refresh_memory();
        self.collect_cpu(app);
        self.collect_memory(app);
        let (user_frac, kernel_frac) = self.cpu_time_split.sample();
        self.cpu_user_frac = user_frac;
        self.cpu_kernel_frac = kernel_frac;
        app.cpu_user_frac = self.cpu_user_frac;
        app.cpu_kernel_frac = self.cpu_kernel_frac;
    }

    /// Refresh all system data and populate the App
    pub fn refresh(&mut self, app: &mut App) {
        if app.paused {
            return; // Z key: freeze display
        }

        // Check if async boot time query has completed
        if self.boot_time_unix.is_none() {
            if let Some(handle) = self.boot_time_pending.take() {
                if handle.is_finished() {
                    self.boot_time_unix = handle.join().ok().flatten();
                } else {
                    self.boot_time_pending = Some(handle);
                }
            }
        }

        // ── Prefetch Win32 data in parallel with sysinfo refresh ──
        // EnumProcesses gives PID list in <1ms, then we launch Win32 batch threads
        // that run concurrently with sysinfo's slower refresh_processes (~100ms).
        let refresh_win_data = self.win_data_cache_ticks == 0 || self.win_data_cache_ticks % 3 == 0;
        let refresh_times = self.win_data_cache_ticks % 3 == 0;

        // Pre-enumerate PIDs via EnumProcesses (<1ms) for parallel Win32 batch calls.
        // I/O counters are fetched every tick; data/users/times only every 3 ticks.
        let pids = winapi::quick_enumerate_pids();

        // Always launch I/O counters in parallel with sysinfo refresh
        let pids_for_io = pids.clone();
        let io_handle = std::thread::spawn(move || winapi::batch_io_counters(&pids_for_io));

        let prefetch_handles = if refresh_win_data {
            let pids_for_data = pids.clone();
            let pids_for_users = pids.clone();
            let data_handle = std::thread::spawn(move || winapi::collect_process_data(&pids_for_data));
            let users_handle = std::thread::spawn(move || winapi::batch_process_users(&pids_for_users));
            let times_handle = if refresh_times {
                let pids_for_times = pids;
                Some(std::thread::spawn(move || winapi::batch_process_times(&pids_for_times)))
            } else {
                None
            };
            Some((data_handle, users_handle, times_handle))
        } else {
            None
        };

        // Refresh sysinfo data (runs concurrently with Win32 prefetch threads above)
        // CPU monitoring is handled natively by NativeCpuMonitor, not sysinfo.
        self.sys.refresh_memory();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );

        // Collect I/O prefetch results
        self.prefetched_io = io_handle.join().unwrap_or_default();

        // Collect optional prefetch results (threads likely finished by now)
        if let Some((data_handle, users_handle, times_handle)) = prefetch_handles {
            self.win_data_cache = data_handle.join().unwrap_or_default();
            self.user_name_cache = users_handle.join().unwrap_or_default();
            if let Some(handle) = times_handle {
                self.process_times_cache = handle.join().unwrap_or_default();
            }
        }

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

        // Compute system-wide DPC/interrupt fracs from per-core data
        let cores = &app.cpu_info.cores;
        if !cores.is_empty() {
            let n = cores.len() as f64;
            app.cpu_dpc_frac = cores.iter().map(|c| c.dpc_frac as f64).sum::<f64>() / n;
            app.cpu_interrupt_frac = cores.iter().map(|c| c.interrupt_frac as f64).sum::<f64>() / n;
        }

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

            // Collect raw I/O rates for processes with connections
            let raw: Vec<(u32, String, f64, f64, u32)> = conn_counts
                .into_iter()
                .map(|(pid, count)| {
                    let (name, io_read, io_write) = app.processes.iter()
                        .find(|p| p.pid == pid)
                        .map(|p| (p.name.clone(), p.io_read_rate, p.io_write_rate))
                        .unwrap_or_else(|| {
                            let name = if pid == 4 { "System".to_string() } else { format!("PID:{}", pid) };
                            (name, 0.0, 0.0)
                        });
                    (pid, name, io_read, io_write, count)
                })
                .collect();

            // Scale per-process I/O by system-wide network rates so that
            // Download/Upload columns reflect actual network direction.
            // GetProcessIoCounters includes ALL I/O (disk + network), so raw
            // io_write_rate is inflated during downloads (saving to disk).
            // Apportioning the true sysinfo network rate by each process's
            // I/O share keeps the columns directionally correct.
            let sys_rx = app.network_info.rx_bytes_per_sec;
            let sys_tx = app.network_info.tx_bytes_per_sec;
            let total_io_read: f64 = raw.iter().map(|(_, _, r, _, _)| *r).sum();
            let total_io_write: f64 = raw.iter().map(|(_, _, _, w, _)| *w).sum();

            let net_procs: Vec<netstat::ProcessNetBandwidth> = raw
                .into_iter()
                .map(|(pid, name, io_read, io_write, count)| {
                    let recv = if total_io_read > 0.0 {
                        sys_rx * (io_read / total_io_read)
                    } else {
                        0.0
                    };
                    let send = if total_io_write > 0.0 {
                        sys_tx * (io_write / total_io_write)
                    } else {
                        0.0
                    };
                    netstat::ProcessNetBandwidth {
                        pid,
                        name,
                        recv_bytes_per_sec: recv,
                        send_bytes_per_sec: send,
                        connection_count: count,
                    }
                })
                .collect();

            // Sort by user's selected Net sort field
            app.net_processes = net_procs;
            app.sort_net_processes();
        }

        // ── GPU per-process data (GPU tab) ──
        if matches!(app.active_tab, crate::app::ProcessTab::Gpu) {
            let mut gpu_procs = self.gpu_collector.collect();
            // Populate process names from sysinfo process list
            for gp in &mut gpu_procs {
                if let Some(proc) = app.processes.iter().find(|p| p.pid == gp.pid) {
                    gp.name = proc.name.clone();
                }
            }
            app.gpu_processes = gpu_procs;
            // Sort by user's selected GPU sort field
            app.sort_gpu_processes();

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

    fn collect_cpu(&mut self, app: &mut App) {
        let samples = self.cpu_monitor.sample();
        let freq = self.cpu_monitor.frequency;

        let cores: Vec<CpuCore> = samples
            .iter()
            .enumerate()
            .map(|(i, s)| CpuCore {
                id: i,
                usage_percent: s.usage_percent,
                frequency_mhz: freq,
                user_frac: s.user_frac,
                kernel_frac: s.kernel_frac,
                dpc_frac: s.dpc_frac,
                interrupt_frac: s.interrupt_frac,
            })
            .collect();

        let total_usage = if cores.is_empty() {
            0.0
        } else {
            cores.iter().map(|c| c.usage_percent).sum::<f32>() / cores.len() as f32
        };

        app.cpu_info = CpuInfo {
            physical_cores: sysinfo::System::physical_core_count().unwrap_or(cores.len()),
            logical_cores: cores.len(),
            total_usage,
            brand: self.cpu_monitor.brand.clone(),
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
        // Lazy-init networks on first use
        let networks = self.networks.get_or_insert_with(Networks::new_with_refreshed_list);
        networks.refresh(true);

        let now = std::time::Instant::now();

        // Sum across all interfaces
        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;
        for (_name, data) in networks.iter() {
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
        let update_names = app.update_process_names;

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

                let pid_u32 = pid.as_u32();

                // When update_process_names is OFF, use cached name/command if available
                let (name, command) = if !update_names {
                    if let Some((cached_name, cached_cmd)) = self.process_name_cache.get(&pid_u32) {
                        (cached_name.clone(), cached_cmd.clone())
                    } else {
                        let cmd = proc_info.cmd();
                        let command = if cmd.is_empty() {
                            proc_info.name().to_string_lossy().to_string()
                        } else {
                            cmd.iter()
                                .map(|s| s.to_string_lossy().to_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        let name = proc_info.name().to_string_lossy().to_string();
                        self.process_name_cache.insert(pid_u32, (name.clone(), command.clone()));
                        (name, command)
                    }
                } else {
                    let cmd = proc_info.cmd();
                    let command = if cmd.is_empty() {
                        proc_info.name().to_string_lossy().to_string()
                    } else {
                        cmd.iter()
                            .map(|s| s.to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    };
                    let name = proc_info.name().to_string_lossy().to_string();
                    self.process_name_cache.insert(pid_u32, (name.clone(), command.clone()));
                    (name, command)
                };

                let ppid = proc_info.parent().map(|p| p.as_u32()).unwrap_or(0);

                // Sanitize cpu_usage: sysinfo can return NaN for inaccessible processes
                let cpu = proc_info.cpu_usage();
                let cpu_usage = if cpu.is_nan() || cpu.is_infinite() { 0.0 } else { cpu };

                (pid.as_u32(), ppid, name, command, proc_info.status(), virt, resident, cpu_usage, mem_pct, proc_info.run_time())
            })
            .collect();

        // Win32 data (priority, users, times, I/O) is pre-fetched in refresh() via parallel threads.
        let all_pids: Vec<u32> = raw_procs.iter().map(|(pid, ..)| *pid).collect();
        let io_counters = std::mem::take(&mut self.prefetched_io);

        self.win_data_cache_ticks += 1;

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

                // Get high-precision CPU time for TIME+ display (from persistent cache)
                let cpu_time_100ns = self.process_times_cache.get(&pid).copied().unwrap_or(0);

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

        // Clean up dead PIDs from prev_io_counters and process_name_cache to prevent memory leak
        self.prev_io_counters.retain(|pid, _| current_pids.contains(pid));
        self.process_name_cache.retain(|pid, _| current_pids.contains(pid));

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
