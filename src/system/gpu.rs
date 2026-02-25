//! GPU-agnostic per-process GPU monitoring via Windows Performance Counters (PDH).
//! Uses the same performance counters that Windows Task Manager reads internally.
//! Works with any WDDM 2.0+ GPU (NVIDIA, AMD, Intel, etc.) on Windows 10 1709+.
//!
//! Counter paths queried:
//!   \GPU Engine(pid_N_*)\Utilization Percentage   — per engine per process
//!   \GPU Process Memory(pid_N_*)\Dedicated Usage  — dedicated GPU memory per process
//!   \GPU Process Memory(pid_N_*)\Shared Usage     — shared GPU memory per process

use std::collections::HashMap;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Per-process GPU usage data (aggregated across all engines/adapters)
#[derive(Debug, Clone, Default)]
pub struct GpuProcessInfo {
    pub pid: u32,
    pub gpu_usage: f64,           // Max engine utilization % across all engines
    pub dedicated_mem: u64,       // Dedicated GPU memory bytes
    pub shared_mem: u64,          // Shared GPU memory bytes
    pub engine_type: String,      // Name of the busiest engine (e.g., "3D", "VideoDecode")
}

/// Overall GPU adapter info
#[derive(Debug, Clone, Default)]
pub struct GpuAdapterInfo {
    pub name: String,
    pub total_dedicated_mem: u64,
    pub total_shared_mem: u64,
    pub overall_usage: f64,       // Overall GPU utilization %
}

// ─── PDH FFI ─────────────────────────────────────────────────────────────────

type PdhQueryHandle = isize;
type PdhCounterHandle = isize;

const PDH_FMT_DOUBLE: u32 = 0x00000200;
const PDH_FMT_LARGE: u32 = 0x00000400;
const PDH_MORE_DATA: u32 = 0x800007D2;

#[repr(C)]
#[allow(non_snake_case)]
struct PDH_FMT_COUNTERVALUE_ITEM_DOUBLE {
    szName: *mut u16,
    value: PDH_FMT_COUNTERVALUE_DOUBLE,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(non_snake_case)]
struct PDH_FMT_COUNTERVALUE_DOUBLE {
    CStatus: u32,
    doubleValue: f64,
}

#[repr(C)]
#[allow(non_snake_case)]
struct PDH_FMT_COUNTERVALUE_ITEM_LARGE {
    szName: *mut u16,
    value: PDH_FMT_COUNTERVALUE_LARGE,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(non_snake_case)]
struct PDH_FMT_COUNTERVALUE_LARGE {
    CStatus: u32,
    largeValue: i64,
}

#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(
        szDataSource: *const u16,
        dwUserData: usize,
        phQuery: *mut PdhQueryHandle,
    ) -> u32;

    fn PdhAddEnglishCounterW(
        hQuery: PdhQueryHandle,
        szFullCounterPath: *const u16,
        dwUserData: usize,
        phCounter: *mut PdhCounterHandle,
    ) -> u32;

    fn PdhCollectQueryData(hQuery: PdhQueryHandle) -> u32;

    fn PdhGetFormattedCounterArrayW(
        hCounter: PdhCounterHandle,
        dwFormat: u32,
        lpdwBufferSize: *mut u32,
        lpdwItemCount: *mut u32,
        ItemBuffer: *mut c_void,
    ) -> u32;

    fn PdhCloseQuery(hQuery: PdhQueryHandle) -> u32;
}

// ─── GPU Collector ───────────────────────────────────────────────────────────

/// Persistent GPU data collector using PDH performance counters.
/// Must be kept alive across ticks to compute utilization deltas.
pub struct GpuCollector {
    query: PdhQueryHandle,
    engine_counter: PdhCounterHandle,
    dedicated_counter: PdhCounterHandle,
    shared_counter: PdhCounterHandle,
    initialized: bool,
    has_sampled_once: bool,
    /// Cached adapter info
    pub adapter_info: GpuAdapterInfo,
}

impl GpuCollector {
    pub fn new() -> Self {
        let mut collector = GpuCollector {
            query: 0,
            engine_counter: 0,
            dedicated_counter: 0,
            shared_counter: 0,
            initialized: false,
            has_sampled_once: false,
            adapter_info: GpuAdapterInfo::default(),
        };
        collector.init();
        collector
    }

    fn init(&mut self) {
        unsafe {
            let status = PdhOpenQueryW(std::ptr::null(), 0, &mut self.query);
            if status != 0 {
                return;
            }

            // Add wildcard counters for all GPU engines and process memory
            let engine_path = to_wide("\\GPU Engine(*)\\Utilization Percentage");
            let ded_path = to_wide("\\GPU Process Memory(*)\\Dedicated Usage");
            let shared_path = to_wide("\\GPU Process Memory(*)\\Shared Usage");

            let s1 = PdhAddEnglishCounterW(
                self.query, engine_path.as_ptr(), 0, &mut self.engine_counter,
            );
            let s2 = PdhAddEnglishCounterW(
                self.query, ded_path.as_ptr(), 0, &mut self.dedicated_counter,
            );
            let s3 = PdhAddEnglishCounterW(
                self.query, shared_path.as_ptr(), 0, &mut self.shared_counter,
            );

            if s1 != 0 && s2 != 0 && s3 != 0 {
                // No GPU counters available (no WDDM 2.0+ GPU, or older Windows)
                PdhCloseQuery(self.query);
                self.query = 0;
                return;
            }

            // First sample (baseline for rate counters)
            PdhCollectQueryData(self.query);
            self.initialized = true;
        }
    }

    /// Collect GPU data. Returns per-process GPU info.
    /// Must be called periodically (e.g., every 1-2 seconds).
    pub fn collect(&mut self) -> Vec<GpuProcessInfo> {
        if !self.initialized || self.query == 0 {
            return Vec::new();
        }

        unsafe {
            let status = PdhCollectQueryData(self.query);
            if status != 0 {
                return Vec::new();
            }

            if !self.has_sampled_once {
                // Need at least 2 samples for rate counters
                self.has_sampled_once = true;
                return Vec::new();
            }

            let mut per_pid: HashMap<u32, GpuProcessInfo> = HashMap::new();

            // Collect engine utilization (per engine per process)
            if self.engine_counter != 0 {
                self.collect_engine_data(&mut per_pid);
            }

            // Collect dedicated memory
            if self.dedicated_counter != 0 {
                self.collect_memory_data(&mut per_pid, false);
            }

            // Collect shared memory
            if self.shared_counter != 0 {
                self.collect_memory_data(&mut per_pid, true);
            }

            // Compute overall GPU usage as sum of all per-process max engine utilization
            // (capped at 100% — better represents aggregate GPU load than just max)
            let overall = per_pid.values()
                .map(|g| g.gpu_usage)
                .sum::<f64>()
                .min(100.0);
            self.adapter_info.overall_usage = overall;

            // Compute total dedicated and shared memory across all GPU-using processes
            self.adapter_info.total_dedicated_mem = per_pid.values().map(|g| g.dedicated_mem).sum();
            self.adapter_info.total_shared_mem = per_pid.values().map(|g| g.shared_mem).sum();

            per_pid.into_values().collect()
        }
    }

    unsafe fn collect_engine_data(&self, per_pid: &mut HashMap<u32, GpuProcessInfo>) {
        let mut buf_size: u32 = 0;
        let mut count: u32 = 0;

        let status = PdhGetFormattedCounterArrayW(
            self.engine_counter,
            PDH_FMT_DOUBLE,
            &mut buf_size,
            &mut count,
            std::ptr::null_mut(),
        );

        if status != PDH_MORE_DATA || buf_size == 0 {
            return;
        }

        let mut buf = vec![0u8; buf_size as usize];
        let status = PdhGetFormattedCounterArrayW(
            self.engine_counter,
            PDH_FMT_DOUBLE,
            &mut buf_size,
            &mut count,
            buf.as_mut_ptr() as *mut c_void,
        );

        if status != 0 {
            return;
        }

        let items = std::slice::from_raw_parts(
            buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_DOUBLE,
            count as usize,
        );

        for item in items {
            if item.value.CStatus != 0 {
                continue;
            }
            let name = read_wide_ptr(item.szName);
            if let Some((pid, engine_type)) = parse_engine_instance(&name) {
                let entry = per_pid.entry(pid).or_insert_with(|| GpuProcessInfo {
                    pid,
                    ..Default::default()
                });
                // Keep the highest utilization engine (like Task Manager)
                if item.value.doubleValue > entry.gpu_usage {
                    entry.gpu_usage = item.value.doubleValue;
                    entry.engine_type = engine_type;
                }
            }
        }
    }

    unsafe fn collect_memory_data(&self, per_pid: &mut HashMap<u32, GpuProcessInfo>, shared: bool) {
        let counter = if shared { self.shared_counter } else { self.dedicated_counter };
        let mut buf_size: u32 = 0;
        let mut count: u32 = 0;

        let status = PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_LARGE,
            &mut buf_size,
            &mut count,
            std::ptr::null_mut(),
        );

        if status != PDH_MORE_DATA || buf_size == 0 {
            return;
        }

        let mut buf = vec![0u8; buf_size as usize];
        let status = PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_LARGE,
            &mut buf_size,
            &mut count,
            buf.as_mut_ptr() as *mut c_void,
        );

        if status != 0 {
            return;
        }

        let items = std::slice::from_raw_parts(
            buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_LARGE,
            count as usize,
        );

        for item in items {
            if item.value.CStatus != 0 {
                continue;
            }
            let name = read_wide_ptr(item.szName);
            if let Some(pid) = parse_memory_instance(&name) {
                let entry = per_pid.entry(pid).or_insert_with(|| GpuProcessInfo {
                    pid,
                    ..Default::default()
                });
                let bytes = item.value.largeValue.max(0) as u64;
                if shared {
                    entry.shared_mem = entry.shared_mem.saturating_add(bytes);
                } else {
                    entry.dedicated_mem = entry.dedicated_mem.saturating_add(bytes);
                }
            }
        }
    }
}

impl Drop for GpuCollector {
    fn drop(&mut self) {
        if self.query != 0 {
            unsafe { PdhCloseQuery(self.query); }
        }
    }
}

// ─── Instance name parsing ───────────────────────────────────────────────────
//
// Engine instances look like:
//   "pid_1234_luid_0x00_0x0000ABCD_phys_0_eng_0_engtype_3D"
//   "pid_5678_luid_0x00_0x0000ABCD_phys_0_eng_1_engtype_VideoDecode"
//
// Memory instances look like:
//   "pid_1234_luid_0x00_0x0000ABCD_phys_0"

fn parse_engine_instance(name: &str) -> Option<(u32, String)> {
    // Extract PID: look for "pid_" prefix
    let pid_start = name.find("pid_")? + 4;
    let pid_end = name[pid_start..].find('_').map(|i| pid_start + i)?;
    let pid: u32 = name[pid_start..pid_end].parse().ok()?;

    // Extract engine type: look for "engtype_"
    let eng_type = if let Some(pos) = name.find("engtype_") {
        name[pos + 8..].to_string()
    } else {
        "Unknown".to_string()
    };

    Some((pid, eng_type))
}

fn parse_memory_instance(name: &str) -> Option<u32> {
    let pid_start = name.find("pid_")? + 4;
    let pid_end = name[pid_start..].find('_').map(|i| pid_start + i).unwrap_or(name.len());
    name[pid_start..pid_end].parse().ok()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn read_wide_ptr(ptr: *mut u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Detect GPU adapter name via DXGI (best-effort, returns first adapter name)
pub fn detect_gpu_adapter_name() -> String {
    // Use WMI via command line as a simple fallback
    // DXGI COM initialization adds complexity — use simple Win32 registry approach
    use std::process::Command;
    let output = Command::new("wmic")
        .args(["path", "Win32_VideoController", "get", "Name", "/format:list"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();
    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                let line = line.trim();
                if let Some(name) = line.strip_prefix("Name=") {
                    let name = name.trim();
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
            "Unknown GPU".to_string()
        }
        Err(_) => "Unknown GPU".to_string(),
    }
}
