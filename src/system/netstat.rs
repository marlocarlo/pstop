//! Per-process network connection enumeration and bandwidth tracking.
//!
//! Enumerates TCP/UDP connections via GetExtendedTcpTable / GetExtendedUdpTable
//! (iphlpapi.dll) to count active connections per PID. Bandwidth (download/upload)
//! is derived from per-process I/O counters (GetProcessIoCounters) which captures
//! all I/O including network. Processes with active connections will show their
//! total I/O throughput as a reasonable bandwidth proxy.
//!
//! This approach works WITHOUT admin, unlike GetPerTcpConnectionEStats.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
//  Public types
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-process aggregated network bandwidth
#[derive(Debug, Clone)]
pub struct ProcessNetBandwidth {
    pub pid: u32,
    pub name: String,
    pub recv_bytes_per_sec: f64,   // download rate (bytes/sec)
    pub send_bytes_per_sec: f64,   // upload rate (bytes/sec)
    pub connection_count: u32,     // active TCP + UDP endpoints
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════════════════════

/// Count active TCP + UDP connections per PID.
/// Returns HashMap<pid, connection_count>. No admin required.
pub fn count_connections_per_pid() -> HashMap<u32, u32> {
    let mut counts: HashMap<u32, u32> = HashMap::new();

    // TCP v4
    for pid in enum_tcp_v4_pids() {
        *counts.entry(pid).or_insert(0) += 1;
    }
    // TCP v6
    for pid in enum_tcp_v6_pids() {
        *counts.entry(pid).or_insert(0) += 1;
    }
    // UDP v4
    for pid in enum_udp_v4_pids() {
        *counts.entry(pid).or_insert(0) += 1;
    }
    // UDP v6
    for pid in enum_udp_v6_pids() {
        *counts.entry(pid).or_insert(0) += 1;
    }

    // Remove system idle process
    counts.remove(&0);

    counts
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Win32 FFI types
// ═══════════════════════════════════════════════════════════════════════════════

const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;
const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
const UDP_TABLE_OWNER_PID: u32 = 1;

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_TCPROW_OWNER_PID {
    dwState: u32,
    dwLocalAddr: u32,
    dwLocalPort: u32,
    dwRemoteAddr: u32,
    dwRemotePort: u32,
    dwOwningPid: u32,
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_TCPTABLE_OWNER_PID {
    dwNumEntries: u32,
    table: [MIB_TCPROW_OWNER_PID; 1],
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_TCP6ROW_OWNER_PID {
    ucLocalAddr: [u8; 16],
    dwLocalScopeId: u32,
    dwLocalPort: u32,
    ucRemoteAddr: [u8; 16],
    dwRemoteScopeId: u32,
    dwRemotePort: u32,
    dwState: u32,
    dwOwningPid: u32,
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_TCP6TABLE_OWNER_PID {
    dwNumEntries: u32,
    table: [MIB_TCP6ROW_OWNER_PID; 1],
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_UDPROW_OWNER_PID {
    dwLocalAddr: u32,
    dwLocalPort: u32,
    dwOwningPid: u32,
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_UDPTABLE_OWNER_PID {
    dwNumEntries: u32,
    table: [MIB_UDPROW_OWNER_PID; 1],
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_UDP6ROW_OWNER_PID {
    ucLocalAddr: [u8; 16],
    dwLocalScopeId: u32,
    dwLocalPort: u32,
    dwOwningPid: u32,
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_UDP6TABLE_OWNER_PID {
    dwNumEntries: u32,
    table: [MIB_UDP6ROW_OWNER_PID; 1],
}

// ═══════════════════════════════════════════════════════════════════════════════
//  FFI declarations (iphlpapi.dll)
// ═══════════════════════════════════════════════════════════════════════════════

#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(
        pTcpTable: *mut u8, pdwSize: *mut u32, bOrder: i32,
        ulAf: u32, TableClass: u32, Reserved: u32,
    ) -> u32;

    fn GetExtendedUdpTable(
        pUdpTable: *mut u8, pdwSize: *mut u32, bOrder: i32,
        ulAf: u32, TableClass: u32, Reserved: u32,
    ) -> u32;
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Connection enumeration (returns PIDs only, no details needed)
// ═══════════════════════════════════════════════════════════════════════════════

fn enum_tcp_v4_pids() -> Vec<u32> {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(
            std::ptr::null_mut(), &mut size, 0,
            AF_INET, TCP_TABLE_OWNER_PID_ALL, 0,
        );
        if size == 0 { return Vec::new(); }

        let mut buf = vec![0u8; size as usize];
        if GetExtendedTcpTable(
            buf.as_mut_ptr(), &mut size, 0,
            AF_INET, TCP_TABLE_OWNER_PID_ALL, 0,
        ) != 0 {
            return Vec::new();
        }

        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        rows.iter().map(|r| r.dwOwningPid).collect()
    }
}

fn enum_tcp_v6_pids() -> Vec<u32> {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(
            std::ptr::null_mut(), &mut size, 0,
            AF_INET6, TCP_TABLE_OWNER_PID_ALL, 0,
        );
        if size == 0 { return Vec::new(); }

        let mut buf = vec![0u8; size as usize];
        if GetExtendedTcpTable(
            buf.as_mut_ptr(), &mut size, 0,
            AF_INET6, TCP_TABLE_OWNER_PID_ALL, 0,
        ) != 0 {
            return Vec::new();
        }

        let table = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        rows.iter().map(|r| r.dwOwningPid).collect()
    }
}

fn enum_udp_v4_pids() -> Vec<u32> {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedUdpTable(
            std::ptr::null_mut(), &mut size, 0,
            AF_INET, UDP_TABLE_OWNER_PID, 0,
        );
        if size == 0 { return Vec::new(); }

        let mut buf = vec![0u8; size as usize];
        if GetExtendedUdpTable(
            buf.as_mut_ptr(), &mut size, 0,
            AF_INET, UDP_TABLE_OWNER_PID, 0,
        ) != 0 {
            return Vec::new();
        }

        let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        rows.iter().map(|r| r.dwOwningPid).collect()
    }
}

fn enum_udp_v6_pids() -> Vec<u32> {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedUdpTable(
            std::ptr::null_mut(), &mut size, 0,
            AF_INET6, UDP_TABLE_OWNER_PID, 0,
        );
        if size == 0 { return Vec::new(); }

        let mut buf = vec![0u8; size as usize];
        if GetExtendedUdpTable(
            buf.as_mut_ptr(), &mut size, 0,
            AF_INET6, UDP_TABLE_OWNER_PID, 0,
        ) != 0 {
            return Vec::new();
        }

        let table = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        rows.iter().map(|r| r.dwOwningPid).collect()
    }
}
