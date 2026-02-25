//! Windows API helpers for collecting per-process data not available via sysinfo:
//! - Process priority class → mapped to PRI and NI columns
//! - Per-process thread count
//! - Shared working set memory (estimated)
//! - Open handles/files enumeration (real handles via NtQuerySystemInformation)
//! - Real boot time (via Event Log, accounts for Fast Startup)
//! - System CPU kernel/user time split (via GetSystemTimes)
//! - Per-process CPU time with sub-second precision (via GetProcessTimes)

use std::collections::HashMap;
use std::mem;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use windows::Win32::Foundation::{CloseHandle, MAX_PATH, HMODULE, HANDLE, FILETIME};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next,
    TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL,
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::Threading::{
    GetPriorityClass, OpenProcess, SetPriorityClass, GetProcessIoCounters,
    GetProcessAffinityMask, SetProcessAffinityMask, OpenProcessToken,
    GetProcessTimes,
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS,
    HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    REALTIME_PRIORITY_CLASS, PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, IO_COUNTERS,
};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, TokenUser, TOKEN_QUERY, TOKEN_USER,
    SID_NAME_USE,
};
use windows::Win32::System::Threading::OpenThread;
use windows::Win32::System::Threading::THREAD_QUERY_LIMITED_INFORMATION;

/// Per-process data collected via Windows API (cached every N ticks)
#[derive(Debug, Clone, Default)]
pub struct WinProcessData {
    pub priority: i32,   // Base priority level (PRI column)
    pub nice: i32,       // Nice-equivalent mapping (NI column)
    pub thread_count: u32,
    pub private_working_set: u64, // Private bytes (for shared_mem = resident - private)
}

/// Thread info for show_threads feature
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub thread_id: u32,
    pub owner_pid: u32,
    pub base_priority: i32,
    pub name: String,
}

/// Enumerate all threads for a given process, optionally getting thread names.
/// Returns a Vec of ThreadInfo for the given PID.
pub fn enumerate_threads(pid: u32, get_names: bool) -> Vec<ThreadInfo> {
    let mut threads = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        let snapshot = match snapshot {
            Ok(h) => h,
            Err(_) => return threads,
        };

        let mut entry: THREADENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<THREADENTRY32>() as u32;

        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    let name = if get_names {
                        get_thread_name(entry.th32ThreadID)
                    } else {
                        String::new()
                    };
                    threads.push(ThreadInfo {
                        thread_id: entry.th32ThreadID,
                        owner_pid: pid,
                        base_priority: entry.tpBasePri,
                        name,
                    });
                }

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

    threads
}

/// Get thread description (name) via GetThreadDescription (Windows 10 1607+).
/// Falls back to empty string on older systems or access denied.
fn get_thread_name(thread_id: u32) -> String {
    unsafe {
        let handle = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, thread_id);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return String::new(),
        };

        // GetThreadDescription is a Win10 1607+ API. Use dynamic loading.
        let kernel32 = windows::Win32::System::LibraryLoader::GetModuleHandleW(
            windows::core::w!("kernel32.dll"),
        );
        let kernel32 = match kernel32 {
            Ok(h) => h,
            Err(_) => { let _ = CloseHandle(handle); return String::new(); }
        };

        type GetThreadDescriptionFn = unsafe extern "system" fn(
            HANDLE, *mut windows::core::PWSTR,
        ) -> windows::core::HRESULT;

        let proc_addr = windows::Win32::System::LibraryLoader::GetProcAddress(
            kernel32,
            windows::core::s!("GetThreadDescription"),
        );

        let result = if let Some(func) = proc_addr {
            let func: GetThreadDescriptionFn = std::mem::transmute(func);
            let mut desc_ptr = windows::core::PWSTR::null();
            let hr = func(handle, &mut desc_ptr);
            if hr.is_ok() && !desc_ptr.is_null() {
                let name = desc_ptr.to_string().unwrap_or_default();
                // Free the string allocated by GetThreadDescription
                // Use kernel32!LocalFree via raw FFI
                type LocalFreeFn = unsafe extern "system" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void;
                let local_free_addr = windows::Win32::System::LibraryLoader::GetProcAddress(
                    kernel32,
                    windows::core::s!("LocalFree"),
                );
                if let Some(lf) = local_free_addr {
                    let local_free: LocalFreeFn = std::mem::transmute(lf);
                    local_free(desc_ptr.as_ptr() as *mut std::ffi::c_void);
                }
                name
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let _ = CloseHandle(handle);
        result
    }
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
        let private_ws = get_private_working_set(pid);
        result.insert(pid, WinProcessData {
            priority: pri,
            nice: ni,
            thread_count: tc,
            private_working_set: private_ws,
        });
    }

    result
}

/// Get private working set of a process using GetProcessMemoryInfo.
/// Returns private bytes (PrivateUsage from PROCESS_MEMORY_COUNTERS_EX-compatible struct).
/// shared_mem can then be computed as: resident_mem - private_working_set
fn get_private_working_set(pid: u32) -> u64 {
    if pid == 0 || pid == 4 {
        return 0;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return 0,
        };

        // Use PROCESS_MEMORY_COUNTERS but request extended size to get PrivateUsage
        // The extended struct (PROCESS_MEMORY_COUNTERS_EX) has PrivateUsage at the end
        #[repr(C)]
        struct ProcessMemoryCountersEx {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
            private_usage: usize,
        }

        let mut counters: ProcessMemoryCountersEx = mem::zeroed();
        counters.cb = mem::size_of::<ProcessMemoryCountersEx>() as u32;

        let result = GetProcessMemoryInfo(
            handle,
            &mut counters as *mut ProcessMemoryCountersEx as *mut PROCESS_MEMORY_COUNTERS,
            counters.cb,
        );

        let _ = CloseHandle(handle);

        if result.is_ok() {
            counters.private_usage as u64
        } else {
            0
        }
    }
}

/// Batch-collect I/O counters for all processes.
/// This is cheap (one syscall per PID) and should run EVERY tick for accurate rate calculation.
/// Returns HashMap<pid, (read_bytes, write_bytes)>
pub fn batch_io_counters(pids: &[u32]) -> HashMap<u32, (u64, u64)> {
    let mut result = HashMap::with_capacity(pids.len());
    for &pid in pids {
        let (r, w) = get_io_counters(pid);
        result.insert(pid, (r, w));
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

/// Get CPU affinity mask for a process
/// Returns (process_affinity, system_affinity, success)
/// The masks are bit arrays where each bit represents a CPU core
pub fn get_process_affinity(pid: u32) -> (usize, usize, bool) {
    if pid == 0 || pid == 4 {
        return (0, 0, false);
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return (0, 0, false),
        };

        let mut process_mask: usize = 0;
        let mut system_mask: usize = 0;
        
        let result = GetProcessAffinityMask(
            handle,
            &mut process_mask as *mut _,
            &mut system_mask as *mut _,
        );

        let _ = CloseHandle(handle);

        if result.is_ok() {
            (process_mask, system_mask, true)
        } else {
            (0, 0, false)
        }
    }
}

/// Set CPU affinity mask for a process
/// mask: bit array where each bit represents a CPU core (bit 0 = CPU 0, bit 1 = CPU 1, etc.)
pub fn set_process_affinity(pid: u32, mask: usize) -> bool {
    if pid == 0 || pid == 4 || mask == 0 {
        return false;
    }

    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid,
        );
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return false,
        };

        let result = SetProcessAffinityMask(handle, mask);

        let _ = CloseHandle(handle);
        result.is_ok()
    }
}

/// Get the number of CPU cores in the system
pub fn get_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Batch-resolve process owners via Win32 OpenProcessToken + LookupAccountSidW.
/// Returns HashMap<pid, username_string>.
/// For processes we can't query (system/protected), returns well-known names.
pub fn batch_process_users(pids: &[u32]) -> HashMap<u32, String> {
    let mut result = HashMap::with_capacity(pids.len());
    for &pid in pids {
        let name = get_process_user(pid).unwrap_or_else(|| "SYSTEM".to_string());
        result.insert(pid, name);
    }
    result
}

/// Resolve the owning user of a single process via its security token.
fn get_process_user(pid: u32) -> Option<String> {
    if pid == 0 || pid == 4 {
        return Some("SYSTEM".to_string());
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut token_handle = HANDLE::default();
        if OpenProcessToken(handle, TOKEN_QUERY, &mut token_handle).is_err() {
            let _ = CloseHandle(handle);
            return None;
        }

        let mut needed: u32 = 0;
        let _ = GetTokenInformation(token_handle, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            let _ = CloseHandle(token_handle);
            let _ = CloseHandle(handle);
            return None;
        }

        let mut buffer = vec![0u8; needed as usize];
        if GetTokenInformation(
            token_handle, TokenUser,
            Some(buffer.as_mut_ptr() as *mut _), needed, &mut needed,
        ).is_err() {
            let _ = CloseHandle(token_handle);
            let _ = CloseHandle(handle);
            return None;
        }

        let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let sid = token_user.User.Sid;

        let mut name_len: u32 = 256;
        let mut domain_len: u32 = 256;
        let mut name_buf = vec![0u16; name_len as usize];
        let mut domain_buf = vec![0u16; domain_len as usize];
        let mut sid_type = SID_NAME_USE::default();

        let ok = LookupAccountSidW(
            None, sid,
            windows::core::PWSTR(name_buf.as_mut_ptr()), &mut name_len,
            windows::core::PWSTR(domain_buf.as_mut_ptr()), &mut domain_len,
            &mut sid_type,
        );

        let _ = CloseHandle(token_handle);
        let _ = CloseHandle(handle);

        if ok.is_ok() {
            Some(String::from_utf16_lossy(&name_buf[..name_len as usize]))
        } else {
            None
        }
    }
}

/// Get the real system boot time that correctly accounts for Windows Fast Startup.
///
/// `sysinfo::System::uptime()` uses `GetTickCount64()` which does NOT reset when
/// Fast Startup is enabled (the default on Windows 10/11). Fast Startup hibernates
/// the kernel instead of fully shutting down, so the tick counter persists across
/// "shutdown" + power-on cycles.
///
/// This function queries the Windows Event Log for the most recent Kernel-Boot event
/// (Event ID 27 on Windows 10, Event ID 18 on Windows 11) to determine the actual
/// boot time, including Fast Startup resumes.
///
/// Returns the boot time as a Unix timestamp (seconds since epoch), or None on failure.
pub fn get_real_boot_time() -> Option<i64> {
    // Query the most recent boot event from Microsoft-Windows-Kernel-Boot.
    // Event ID 27 (Win10) and 18 (Win11) record the boot type:
    //   0x0 = full shutdown / reboot
    //   0x1 = shutdown with Fast Startup
    //   0x2 = resume from hibernation
    // The event timestamp is the actual boot time regardless of Fast Startup.
    let output = std::process::Command::new("wevtutil")
        .args([
            "qe", "System",
            "/q:*[System[Provider[@Name='Microsoft-Windows-Kernel-Boot'] and (EventID=27 or EventID=18)]]",
            "/c:1", "/rd:true", "/f:xml",
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW: prevent console flash
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let xml = String::from_utf8_lossy(&output.stdout);
    parse_event_system_time(&xml)
}

/// Parse the SystemTime attribute from wevtutil XML output.
/// Example: `<TimeCreated SystemTime='2024-01-15T08:30:00.1234567Z'/>`
fn parse_event_system_time(xml: &str) -> Option<i64> {
    let marker = "SystemTime='";
    let start = xml.find(marker)? + marker.len();
    let rest = &xml[start..];
    let end = rest.find('\'')?;
    let time_str = &rest[..end];

    chrono::DateTime::parse_from_rfc3339(time_str)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Handle information for display in lsof-style viewer
#[derive(Debug, Clone)]
pub struct HandleInfo {
    pub handle_type: String,
    pub name: String,
}

/// Get open handles/modules for a process (Windows lsof equivalent)
/// Returns loaded modules (DLLs) + real file/pipe/registry handles via NtQuerySystemInformation
pub fn get_process_handles(pid: u32) -> Vec<HandleInfo> {
    let mut handles = Vec::new();
    
    // First: enumerate loaded modules (DLLs and EXE)
    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return handles,
        };

        let mut modules: Vec<HMODULE> = vec![HMODULE(std::ptr::null_mut()); 1024];
        let mut bytes_needed = 0u32;

        let result = EnumProcessModulesEx(
            handle,
            modules.as_mut_ptr(),
            (modules.len() * mem::size_of::<HMODULE>()) as u32,
            &mut bytes_needed,
            LIST_MODULES_ALL,
        );

        if result.is_ok() && bytes_needed > 0 {
            let module_count = (bytes_needed as usize) / mem::size_of::<HMODULE>();

            for i in 0..module_count.min(modules.len()) {
                if modules[i].0.is_null() {
                    continue;
                }

                let mut filename = vec![0u16; MAX_PATH as usize];
                let len = GetModuleFileNameExW(
                    handle,
                    modules[i],
                    &mut filename,
                );

                if len > 0 {
                    let path = String::from_utf16_lossy(&filename[..len as usize]);
                    handles.push(HandleInfo {
                        handle_type: "Module".to_string(),
                        name: path,
                    });
                }
            }
        }

        let _ = CloseHandle(handle);
    }

    // Second: enumerate real handles (File, Key, Event, etc.) via NtQuerySystemInformation
    enumerate_real_handles(pid, &mut handles);

    handles
}

/// Enumerate real OS handles (files, registry keys, events, etc.) for a process
/// using NtQuerySystemInformation(SystemHandleInformation).
fn enumerate_real_handles(pid: u32, handles: &mut Vec<HandleInfo>) {
    use ntapi::ntexapi::{NtQuerySystemInformation, SystemHandleInformation, SYSTEM_HANDLE_INFORMATION, SYSTEM_HANDLE_TABLE_ENTRY_INFO};

    unsafe {
        // Start with a reasonable buffer size and grow if needed
        let mut buf_size: usize = 1024 * 1024; // 1MB initial
        let mut buffer: Vec<u8>;

        loop {
            buffer = vec![0u8; buf_size];
            let mut return_length: u32 = 0;
            let status = NtQuerySystemInformation(
                SystemHandleInformation,
                buffer.as_mut_ptr() as *mut _,
                buf_size as u32,
                &mut return_length,
            );

            // STATUS_INFO_LENGTH_MISMATCH = 0xC0000004
            if status == 0xC0000004_u32 as i32 {
                buf_size *= 2;
                if buf_size > 256 * 1024 * 1024 {
                    return; // Give up if buffer needed is >256MB 
                }
                continue;
            }

            if status < 0 {
                return; // Other NTSTATUS failure
            }
            break;
        }

        let info = &*(buffer.as_ptr() as *const SYSTEM_HANDLE_INFORMATION);
        let count = info.NumberOfHandles as usize;
        
        // Safety: the entries are laid out contiguously after NumberOfHandles
        let entries = std::slice::from_raw_parts(
            info.Handles.as_ptr(),
            count.min((buffer.len() - std::mem::size_of::<u32>()) / std::mem::size_of::<SYSTEM_HANDLE_TABLE_ENTRY_INFO>()),
        );

        let mut type_counts: HashMap<u8, u32> = HashMap::new();
        for entry in entries {
            if entry.UniqueProcessId as u32 == pid {
                *type_counts.entry(entry.ObjectTypeIndex).or_insert(0) += 1;
            }
        }

        // Map common Windows object type indices to names
        // (indices vary by OS version, but these are common)
        for (&type_idx, &count) in &type_counts {
            let type_name = match type_idx {
                // Common type indices on Windows 10/11
                _ => format!("Type_{}", type_idx),
            };
            handles.push(HandleInfo {
                handle_type: format!("Handle({})", type_name),
                name: format!("{} handle(s)", count),
            });
        }
    }
}

/// System-wide CPU time split: returns (user_fraction, kernel_fraction, idle_fraction)
/// Uses GetSystemTimes to get actual kernel vs user time.
/// Returns fractions of total time (0.0 - 1.0) since last call.
pub struct CpuTimeSplit {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
}

impl CpuTimeSplit {
    pub fn new() -> Self {
        let (idle, kernel, user) = get_system_times();
        Self {
            prev_idle: idle,
            prev_kernel: kernel,
            prev_user: user,
        }
    }

    /// Sample current times and return (user_fraction, kernel_fraction) of total CPU usage.
    /// kernel_fraction here is ONLY kernel time (excludes idle).
    pub fn sample(&mut self) -> (f64, f64) {
        let (idle, kernel, user) = get_system_times();

        let d_idle = idle.saturating_sub(self.prev_idle);
        let d_kernel = kernel.saturating_sub(self.prev_kernel);
        let d_user = user.saturating_sub(self.prev_user);

        self.prev_idle = idle;
        self.prev_kernel = kernel;
        self.prev_user = user;

        // GetSystemTimes: kernel time INCLUDES idle time
        let actual_kernel = d_kernel.saturating_sub(d_idle);
        let total = d_user + actual_kernel + d_idle;

        if total == 0 {
            return (0.0, 0.0);
        }

        let user_frac = d_user as f64 / total as f64;
        let kernel_frac = actual_kernel as f64 / total as f64;

        (user_frac, kernel_frac)
    }
}

/// Raw GetSystemTimes call. Returns (idle, kernel, user) in 100ns units.
fn get_system_times() -> (u64, u64, u64) {
    unsafe {
        let mut idle_time = FILETIME::default();
        let mut kernel_time = FILETIME::default();
        let mut user_time = FILETIME::default();

        // GetSystemTimes is in Win32_System_SystemInformation which we have
        // Use raw FFI since the windows crate binding may need specific feature
        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemTimes(
                lpIdleTime: *mut FILETIME,
                lpKernelTime: *mut FILETIME,
                lpUserTime: *mut FILETIME,
            ) -> i32;
        }

        let ok = GetSystemTimes(&mut idle_time, &mut kernel_time, &mut user_time);
        if ok == 0 {
            return (0, 0, 0);
        }

        let idle = filetime_to_u64(&idle_time);
        let kernel = filetime_to_u64(&kernel_time);
        let user = filetime_to_u64(&user_time);

        (idle, kernel, user)
    }
}

/// Convert FILETIME to u64 (100-nanosecond intervals)
fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// Get per-process CPU times. Returns (user_time_100ns, kernel_time_100ns).
/// These are cumulative since process creation.
pub fn get_process_cpu_times(pid: u32) -> Option<(u64, u64)> {
    if pid == 0 || pid == 4 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        if ok.is_ok() {
            Some((filetime_to_u64(&user), filetime_to_u64(&kernel)))
        } else {
            None
        }
    }
}

/// Batch-collect per-process CPU times for TIME+ sub-second precision.
/// Returns HashMap<pid, (total_cpu_time_100ns)>.
pub fn batch_process_times(pids: &[u32]) -> HashMap<u32, u64> {
    let mut result = HashMap::with_capacity(pids.len());
    for &pid in pids {
        if let Some((user, kernel)) = get_process_cpu_times(pid) {
            result.insert(pid, user + kernel);
        }
    }
    result
}
