//! Per-process network bandwidth tracking for Windows.
//!
//! Enumerates TCP/UDP connections via GetExtendedTcpTable / GetExtendedUdpTable
//! (iphlpapi.dll), then uses GetPerTcpConnectionEStats to measure per-connection
//! byte counters for live download/upload rates. Admin privileges are required
//! for bandwidth data; without admin, connection counts are still tracked.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

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

/// Stateful tracker — persists between polls to compute rate deltas.
pub struct NetBandwidthTracker {
    /// Per-connection cumulative byte counts from previous poll
    prev_bytes: HashMap<ConnKey, (u64, u64)>,
    /// Connections for which we've enabled EStats collection
    enabled_set: HashSet<ConnKey>,
    /// Admin-level EStats availability: None = untested, Some(true/false) = known
    admin_ok: Option<bool>,
    /// Timestamp of last poll
    last_poll: Instant,
}

impl NetBandwidthTracker {
    pub fn new() -> Self {
        Self {
            prev_bytes: HashMap::new(),
            enabled_set: HashSet::new(),
            admin_ok: None,
            last_poll: Instant::now(),
        }
    }

    /// Whether per-connection byte stats are available (requires admin).
    pub fn has_bandwidth_data(&self) -> bool {
        self.admin_ok != Some(false)
    }

    /// Poll all connections and compute per-process bandwidth.
    /// `pid_names` maps PID → process name for display.
    pub fn collect(&mut self, pid_names: &HashMap<u32, String>) -> Vec<ProcessNetBandwidth> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_poll).as_secs_f64();
        if elapsed < 0.05 {
            return Vec::new();
        }

        let tcp4 = enum_tcp_v4();
        let tcp6 = enum_tcp_v6();
        let udp4 = count_udp_v4();
        let udp6 = count_udp_v6();

        let try_stats = self.admin_ok != Some(false);
        let mut current_keys: HashSet<ConnKey> = HashSet::new();
        let mut acc: HashMap<u32, Accum> = HashMap::new();

        // ── TCP v4 ──
        for c in &tcp4 {
            let key = ConnKey::V4(c.local_addr, c.local_port, c.remote_addr, c.remote_port);
            current_keys.insert(key.clone());

            let e = acc.entry(c.pid).or_insert_with(|| Accum::new(
                pid_names.get(&c.pid).cloned().unwrap_or_else(|| fallback_name(c.pid)),
            ));
            e.tcp += 1;

            // Only track bandwidth for ESTABLISHED (state 5) connections
            if try_stats && c.state == 5 {
                self.probe_v4(c, &key, e);
            }
        }

        // ── TCP v6 ──
        for c in &tcp6 {
            let key = ConnKey::V6(c.local_addr, c.local_port, c.remote_addr, c.remote_port);
            current_keys.insert(key.clone());

            let e = acc.entry(c.pid).or_insert_with(|| Accum::new(
                pid_names.get(&c.pid).cloned().unwrap_or_else(|| fallback_name(c.pid)),
            ));
            e.tcp += 1;

            if try_stats && c.state == 5 {
                self.probe_v6(c, &key, e);
            }
        }

        // ── UDP endpoints (count only — no per-connection stats for UDP) ──
        for (&pid, &cnt) in &udp4 {
            acc.entry(pid).or_insert_with(|| Accum::new(
                pid_names.get(&pid).cloned().unwrap_or_else(|| fallback_name(pid)),
            )).udp += cnt;
        }
        for (&pid, &cnt) in &udp6 {
            acc.entry(pid).or_insert_with(|| Accum::new(
                pid_names.get(&pid).cloned().unwrap_or_else(|| fallback_name(pid)),
            )).udp += cnt;
        }

        // Prune stale connection tracking
        self.prev_bytes.retain(|k, _| current_keys.contains(k));
        self.enabled_set.retain(|k| current_keys.contains(k));

        // Build output (skip System Idle pid 0)
        let mut out: Vec<ProcessNetBandwidth> = acc
            .into_iter()
            .filter(|(pid, _)| *pid != 0)
            .map(|(pid, a)| ProcessNetBandwidth {
                pid,
                name: a.name,
                recv_bytes_per_sec: a.din as f64 / elapsed,
                send_bytes_per_sec: a.dout as f64 / elapsed,
                connection_count: a.tcp + a.udp,
            })
            .collect();

        // Sort: highest total bandwidth first, then by connection count
        out.sort_by(|a, b| {
            let ar = a.recv_bytes_per_sec + a.send_bytes_per_sec;
            let br = b.recv_bytes_per_sec + b.send_bytes_per_sec;
            br.partial_cmp(&ar)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.connection_count.cmp(&a.connection_count))
        });

        self.last_poll = now;
        out
    }

    // ── Private: enable + read per-connection stats (IPv4) ──

    fn probe_v4(&mut self, c: &TcpV4, key: &ConnKey, acc: &mut Accum) {
        let row = MIB_TCPROW {
            dwState: c.state,
            dwLocalAddr: c.local_addr,
            dwLocalPort: c.local_port,
            dwRemoteAddr: c.remote_addr,
            dwRemotePort: c.remote_port,
        };

        // Enable stats if not already done
        if !self.enabled_set.contains(key) {
            if set_estats(&row) {
                self.enabled_set.insert(key.clone());
                if self.admin_ok.is_none() {
                    self.admin_ok = Some(true);
                }
            } else if self.admin_ok.is_none() {
                self.admin_ok = Some(false);
                return;
            }
        }

        // Read stats
        if self.admin_ok == Some(true) {
            if let Some((bi, bo)) = get_estats(&row) {
                let prev = self.prev_bytes.get(key).copied().unwrap_or((0, 0));
                acc.din += bi.saturating_sub(prev.0);
                acc.dout += bo.saturating_sub(prev.1);
                self.prev_bytes.insert(key.clone(), (bi, bo));
            }
        }
    }

    // ── Private: enable + read per-connection stats (IPv6) ──

    fn probe_v6(&mut self, c: &TcpV6, key: &ConnKey, acc: &mut Accum) {
        let row = MIB_TCP6ROW {
            State: c.state,
            LocalAddr: c.local_addr,
            dwLocalScopeId: c.local_scope_id,
            dwLocalPort: c.local_port,
            RemoteAddr: c.remote_addr,
            dwRemoteScopeId: c.remote_scope_id,
            dwRemotePort: c.remote_port,
        };

        if !self.enabled_set.contains(key) {
            if set_estats_v6(&row) {
                self.enabled_set.insert(key.clone());
                if self.admin_ok.is_none() {
                    self.admin_ok = Some(true);
                }
            } else if self.admin_ok.is_none() {
                self.admin_ok = Some(false);
                return;
            }
        }

        if self.admin_ok == Some(true) {
            if let Some((bi, bo)) = get_estats_v6(&row) {
                let prev = self.prev_bytes.get(key).copied().unwrap_or((0, 0));
                acc.din += bi.saturating_sub(prev.0);
                acc.dout += bo.saturating_sub(prev.1);
                self.prev_bytes.insert(key.clone(), (bi, bo));
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Internal helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-PID accumulator used during collection
struct Accum {
    name: String,
    din: u64,   // delta bytes in (this poll)
    dout: u64,  // delta bytes out (this poll)
    tcp: u32,
    udp: u32,
}

impl Accum {
    fn new(name: String) -> Self {
        Self { name, din: 0, dout: 0, tcp: 0, udp: 0 }
    }
}

fn fallback_name(pid: u32) -> String {
    match pid {
        0 => "System Idle".to_string(),
        4 => "System".to_string(),
        _ => format!("PID:{}", pid),
    }
}

/// Unique key for a TCP connection (raw network-byte-order values)
#[derive(Hash, Eq, PartialEq, Clone)]
enum ConnKey {
    V4(u32, u32, u32, u32),           // (local_addr, local_port, remote_addr, remote_port)
    V6([u8; 16], u32, [u8; 16], u32), // (local_addr, local_port, remote_addr, remote_port)
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Internal connection structs (raw from GetExtendedTcpTable, NBO)
// ═══════════════════════════════════════════════════════════════════════════════

struct TcpV4 {
    state: u32,
    local_addr: u32,
    local_port: u32,
    remote_addr: u32,
    remote_port: u32,
    pid: u32,
}

struct TcpV6 {
    state: u32,
    local_addr: [u8; 16],
    local_scope_id: u32,
    local_port: u32,
    remote_addr: [u8; 16],
    remote_scope_id: u32,
    remote_port: u32,
    pid: u32,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Win32 FFI types
// ═══════════════════════════════════════════════════════════════════════════════

const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;
const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
const UDP_TABLE_OWNER_PID: u32 = 1;
const TCP_ESTATS_DATA: i32 = 0; // TcpConnectionEstatsData

// ── GetExtendedTcpTable / GetExtendedUdpTable row structs ──

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

// ── Per-connection EStats structs ──

/// Row passed to Set/GetPerTcpConnectionEStats (IPv4)
#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_TCPROW {
    dwState: u32,
    dwLocalAddr: u32,
    dwLocalPort: u32,
    dwRemoteAddr: u32,
    dwRemotePort: u32,
}

/// Row passed to Set/GetPerTcp6ConnectionEStats (IPv6)
#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
struct MIB_TCP6ROW {
    State: u32,
    LocalAddr: [u8; 16],
    dwLocalScopeId: u32,
    dwLocalPort: u32,
    RemoteAddr: [u8; 16],
    dwRemoteScopeId: u32,
    dwRemotePort: u32,
}

/// Enable/disable data collection for a connection
#[repr(C)]
struct TcpEstatsDataRw {
    enable_collection: u8,
}

/// Read-only data: cumulative bytes in/out per connection
#[repr(C)]
#[allow(non_snake_case, dead_code)]
struct TcpEstatsDataRod {
    DataBytesOut: u64,
    DataSegsOut: u64,
    DataBytesIn: u64,
    DataSegsIn: u64,
    SegsOut: u64,
    SegsIn: u64,
    SoftErrors: u64,
    SoftErrorReason: u64,
    SndUna: u32,
    SndNxt: u32,
    SndMax: u32,
    ThreshBytesAcked: u64,
    PipelinedBytesAcked: u64,
    ThreshSegsAcked: u32,
    PipelinedSegsAcked: u32,
    RcvNxt: u32,
    ThreshBytesReceived: u64,
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

    fn SetPerTcpConnectionEStats(
        Row: *const MIB_TCPROW, EstatsType: i32,
        Rw: *const u8, RwVersion: u32, RwSize: u32, Offset: u32,
    ) -> u32;

    fn GetPerTcpConnectionEStats(
        Row: *const MIB_TCPROW, EstatsType: i32,
        Rw: *mut u8, RwVersion: u32, RwSize: u32,
        Ros: *mut u8, RosVersion: u32, RosSize: u32,
        Rod: *mut u8, RodVersion: u32, RodSize: u32,
    ) -> u32;

    fn SetPerTcp6ConnectionEStats(
        Row: *const MIB_TCP6ROW, EstatsType: i32,
        Rw: *const u8, RwVersion: u32, RwSize: u32, Offset: u32,
    ) -> u32;

    fn GetPerTcp6ConnectionEStats(
        Row: *const MIB_TCP6ROW, EstatsType: i32,
        Rw: *mut u8, RwVersion: u32, RwSize: u32,
        Ros: *mut u8, RosVersion: u32, RosSize: u32,
        Rod: *mut u8, RodVersion: u32, RodSize: u32,
    ) -> u32;
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Connection enumeration
// ═══════════════════════════════════════════════════════════════════════════════

fn enum_tcp_v4() -> Vec<TcpV4> {
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
        rows.iter().map(|r| TcpV4 {
            state: r.dwState,
            local_addr: r.dwLocalAddr,
            local_port: r.dwLocalPort,
            remote_addr: r.dwRemoteAddr,
            remote_port: r.dwRemotePort,
            pid: r.dwOwningPid,
        }).collect()
    }
}

fn enum_tcp_v6() -> Vec<TcpV6> {
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
        rows.iter().map(|r| TcpV6 {
            state: r.dwState,
            local_addr: r.ucLocalAddr,
            local_scope_id: r.dwLocalScopeId,
            local_port: r.dwLocalPort,
            remote_addr: r.ucRemoteAddr,
            remote_scope_id: r.dwRemoteScopeId,
            remote_port: r.dwRemotePort,
            pid: r.dwOwningPid,
        }).collect()
    }
}

fn count_udp_v4() -> HashMap<u32, u32> {
    let mut counts = HashMap::new();
    unsafe {
        let mut size: u32 = 0;
        GetExtendedUdpTable(
            std::ptr::null_mut(), &mut size, 0,
            AF_INET, UDP_TABLE_OWNER_PID, 0,
        );
        if size == 0 { return counts; }

        let mut buf = vec![0u8; size as usize];
        if GetExtendedUdpTable(
            buf.as_mut_ptr(), &mut size, 0,
            AF_INET, UDP_TABLE_OWNER_PID, 0,
        ) != 0 {
            return counts;
        }

        let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        for r in rows {
            *counts.entry(r.dwOwningPid).or_insert(0) += 1;
        }
    }
    counts
}

fn count_udp_v6() -> HashMap<u32, u32> {
    let mut counts = HashMap::new();
    unsafe {
        let mut size: u32 = 0;
        GetExtendedUdpTable(
            std::ptr::null_mut(), &mut size, 0,
            AF_INET6, UDP_TABLE_OWNER_PID, 0,
        );
        if size == 0 { return counts; }

        let mut buf = vec![0u8; size as usize];
        if GetExtendedUdpTable(
            buf.as_mut_ptr(), &mut size, 0,
            AF_INET6, UDP_TABLE_OWNER_PID, 0,
        ) != 0 {
            return counts;
        }

        let table = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        for r in rows {
            *counts.entry(r.dwOwningPid).or_insert(0) += 1;
        }
    }
    counts
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Per-connection EStats (admin-only)
// ═══════════════════════════════════════════════════════════════════════════════

/// Enable data collection on a TCP v4 connection. Returns true on success.
fn set_estats(row: &MIB_TCPROW) -> bool {
    unsafe {
        let rw = TcpEstatsDataRw { enable_collection: 1 };
        SetPerTcpConnectionEStats(
            row as *const MIB_TCPROW,
            TCP_ESTATS_DATA,
            &rw as *const TcpEstatsDataRw as *const u8,
            0,
            std::mem::size_of::<TcpEstatsDataRw>() as u32,
            0,
        ) == 0
    }
}

/// Read cumulative bytes (in, out) for a TCP v4 connection.
fn get_estats(row: &MIB_TCPROW) -> Option<(u64, u64)> {
    unsafe {
        let mut rod = std::mem::zeroed::<TcpEstatsDataRod>();
        let ret = GetPerTcpConnectionEStats(
            row as *const MIB_TCPROW,
            TCP_ESTATS_DATA,
            std::ptr::null_mut(), 0, 0,
            std::ptr::null_mut(), 0, 0,
            &mut rod as *mut TcpEstatsDataRod as *mut u8,
            0,
            std::mem::size_of::<TcpEstatsDataRod>() as u32,
        );
        if ret == 0 {
            Some((rod.DataBytesIn, rod.DataBytesOut))
        } else {
            None
        }
    }
}

/// Enable data collection on a TCP v6 connection.
fn set_estats_v6(row: &MIB_TCP6ROW) -> bool {
    unsafe {
        let rw = TcpEstatsDataRw { enable_collection: 1 };
        SetPerTcp6ConnectionEStats(
            row as *const MIB_TCP6ROW,
            TCP_ESTATS_DATA,
            &rw as *const TcpEstatsDataRw as *const u8,
            0,
            std::mem::size_of::<TcpEstatsDataRw>() as u32,
            0,
        ) == 0
    }
}

/// Read cumulative bytes (in, out) for a TCP v6 connection.
fn get_estats_v6(row: &MIB_TCP6ROW) -> Option<(u64, u64)> {
    unsafe {
        let mut rod = std::mem::zeroed::<TcpEstatsDataRod>();
        let ret = GetPerTcp6ConnectionEStats(
            row as *const MIB_TCP6ROW,
            TCP_ESTATS_DATA,
            std::ptr::null_mut(), 0, 0,
            std::ptr::null_mut(), 0, 0,
            &mut rod as *mut TcpEstatsDataRod as *mut u8,
            0,
            std::mem::size_of::<TcpEstatsDataRod>() as u32,
        );
        if ret == 0 {
            Some((rod.DataBytesIn, rod.DataBytesOut))
        } else {
            None
        }
    }
}
