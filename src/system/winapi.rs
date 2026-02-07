//! Windows API helpers for collecting per-process data not available via sysinfo:
//! - Process priority class → mapped to PRI and NI columns
//! - Per-process thread count
//! - Shared working set memory (estimated)

use std::collections::HashMap;
use std::mem;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next,
    TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows::Win32::System::Threading::{
    GetPriorityClass, OpenProcess, SetPriorityClass, GetProcessIoCounters,
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS,
    HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    REALTIME_PRIORITY_CLASS, PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
    IO_COUNTERS,
};

/// Per-process data collected via Windows API
#[derive(Debug, Clone, Default)]
pub struct WinProcessData {
    pub priority: i32,   // Base priority level (PRI column)
    pub nice: i32,       // Nice-equivalent mapping (NI column)
    pub thread_count: u32,
    // I/O counters (cumulative bytes)
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// Batch-collect Windows-specific process data for all running processes.
/// This is efficient: takes one thread snapshot for all threads, then queries
/// each process for priority individually.
pub fn collect_process_data(pids: &[u32]) -> HashMap<u32, WinProcessData> {
    let thread_counts = count_all_threads();
    let mut result = HashMap::with_capacity(pids.len());

    for &pid in pids {
        let tc = thread_counts.get(&pid).copied().unwrap_or(1);
        let (pri, ni) = get_priority(pid);
        let (io_read, io_write) = get_io_counters(pid);
        result.insert(pid, WinProcessData {
            priority: pri,
            nice: ni,
            thread_count: tc,
            io_read_bytes: io_read,
            io_write_bytes: io_write,
        });
    }

    result
}

/// Count threads per process by taking a system-wide thread snapshot.
/// Returns HashMap<owning_pid, thread_count>.
fn count_all_threads() -> HashMap<u32, u32> {
    let mut map: HashMap<u32, u32> = HashMap::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        let snapshot = match snapshot {
            Ok(h) => h,
            Err(_) => return map,
        };

        let mut entry: THREADENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<THREADENTRY32>() as u32;

        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                *map.entry(entry.th32OwnerProcessID).or_insert(0) += 1;

                let mut next_entry: THREADENTRY32 = mem::zeroed();
                next_entry.dwSize = mem::size_of::<THREADENTRY32>() as u32;
                if Thread32Next(snapshot, &mut next_entry).is_err() {
                    break;
                }
                entry = next_entry;
            }
        }

        let _ = CloseHandle(snapshot);
    }

    map
}

/// Get process priority class and map to PRI (base priority) and NI (nice-equivalent).
///
/// Windows priority classes map:
///   IDLE_PRIORITY_CLASS         → PRI 4,  NI 19
///   BELOW_NORMAL_PRIORITY_CLASS → PRI 6,  NI 10
///   NORMAL_PRIORITY_CLASS       → PRI 8,  NI 0
///   ABOVE_NORMAL_PRIORITY_CLASS → PRI 10, NI -5
///   HIGH_PRIORITY_CLASS         → PRI 13, NI -10
///   REALTIME_PRIORITY_CLASS     → PRI 24, NI -20
fn get_priority(pid: u32) -> (i32, i32) {
    if pid == 0 || pid == 4 {
        // System Idle Process / System — can't open
        return (0, 0);
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return (8, 0), // Default to NORMAL if can't access
        };

        let pclass = GetPriorityClass(handle);
        let _ = CloseHandle(handle);

        map_priority_class(pclass)
    }
}

/// Map Win32 priority class DWORD to (PRI, NI) tuple
fn map_priority_class(pclass: u32) -> (i32, i32) {
    match pclass {
        x if x == IDLE_PRIORITY_CLASS.0         => (4, 19),
        x if x == BELOW_NORMAL_PRIORITY_CLASS.0 => (6, 10),
        x if x == NORMAL_PRIORITY_CLASS.0       => (8, 0),
        x if x == ABOVE_NORMAL_PRIORITY_CLASS.0 => (10, -5),
        x if x == HIGH_PRIORITY_CLASS.0         => (13, -10),
        x if x == REALTIME_PRIORITY_CLASS.0     => (24, -20),
        _ => (8, 0), // Unknown → NORMAL
    }
}

/// Get I/O counters for a process (cumulative bytes read/written)
/// Returns (read_bytes, write_bytes)
fn get_io_counters(pid: u32) -> (u64, u64) {
    if pid == 0 || pid == 4 {
        // System Idle Process / System — can't open
        return (0, 0);
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return (0, 0),
        };

        let mut counters: IO_COUNTERS = mem::zeroed();
        let result = GetProcessIoCounters(handle, &mut counters as *mut _);
        
        let _ = CloseHandle(handle);

        if result.is_ok() {
            (counters.ReadTransferCount, counters.WriteTransferCount)
        } else {
            (0, 0)
        }
    }
}

/// Increase priority of a process (F7 = Nice-, raise priority).
/// Moves one priority class up: IDLE → BELOW_NORMAL → NORMAL → ABOVE_NORMAL → HIGH
pub fn raise_priority(pid: u32) -> bool {
    change_priority(pid, true)
}

/// Decrease priority of a process (F8 = Nice+, lower priority).
/// Moves one priority class down: HIGH → ABOVE_NORMAL → NORMAL → BELOW_NORMAL → IDLE
pub fn lower_priority(pid: u32) -> bool {
    change_priority(pid, false)
}

fn change_priority(pid: u32, raise: bool) -> bool {
    if pid == 0 || pid == 4 {
        return false;
    }

    unsafe {
        // Need both QUERY (to read current) and SET (to change)
        let handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid,
        );
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return false,
        };

        let current = GetPriorityClass(handle);

        // Priority ladder (excluding REALTIME for safety):
        // IDLE → BELOW_NORMAL → NORMAL → ABOVE_NORMAL → HIGH
        let ladder = [
            IDLE_PRIORITY_CLASS,
            BELOW_NORMAL_PRIORITY_CLASS,
            NORMAL_PRIORITY_CLASS,
            ABOVE_NORMAL_PRIORITY_CLASS,
            HIGH_PRIORITY_CLASS,
        ];

        let current_idx = ladder.iter().position(|c| c.0 == current);
        let new_class = match current_idx {
            Some(idx) => {
                if raise {
                    if idx + 1 < ladder.len() { Some(ladder[idx + 1]) } else { None }
                } else {
                    if idx > 0 { Some(ladder[idx - 1]) } else { None }
                }
            }
            None => None,
        };

        let success = if let Some(nc) = new_class {
            SetPriorityClass(handle, nc).is_ok()
        } else {
            false
        };

        let _ = CloseHandle(handle);
        success
    }
}
