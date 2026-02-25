//! Real per-process network connection enumeration via Win32 APIs.
//! Uses GetExtendedTcpTable / GetExtendedUdpTable from iphlpapi.dll
//! to enumerate all TCP (v4/v6) and UDP (v4/v6) connections with owning PIDs.
//! This is the same technique used by psnet, netstat -b, and Resource Monitor.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnProto {
    Tcp,
    Udp,
}

impl ConnProto {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
    Unknown,
}

impl TcpState {
    fn from_raw(val: u32) -> Self {
        match val {
            1 => Self::Closed,
            2 => Self::Listen,
            3 => Self::SynSent,
            4 => Self::SynReceived,
            5 => Self::Established,
            6 => Self::FinWait1,
            7 => Self::FinWait2,
            8 => Self::CloseWait,
            9 => Self::Closing,
            10 => Self::LastAck,
            11 => Self::TimeWait,
            12 => Self::DeleteTcb,
            _ => Self::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Listen => "LISTEN",
            Self::SynSent => "SYN_SENT",
            Self::SynReceived => "SYN_RCVD",
            Self::Established => "ESTAB",
            Self::FinWait1 => "FIN_WAIT1",
            Self::FinWait2 => "FIN_WAIT2",
            Self::CloseWait => "CLOSE_WAIT",
            Self::Closing => "CLOSING",
            Self::LastAck => "LAST_ACK",
            Self::TimeWait => "TIME_WAIT",
            Self::DeleteTcb => "DELETE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetConnection {
    pub proto: ConnProto,
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: Option<IpAddr>,
    pub remote_port: Option<u16>,
    pub state: Option<TcpState>,
    pub pid: u32,
    pub process_name: String,
}

impl NetConnection {
    pub fn local_str(&self) -> String {
        format!("{}:{}", self.local_addr, self.local_port)
    }

    pub fn remote_str(&self) -> String {
        match (&self.remote_addr, self.remote_port) {
            (Some(addr), Some(port)) => format!("{}:{}", addr, port),
            _ => "*:*".to_string(),
        }
    }

    /// Return well-known service label for the remote port
    pub fn service_label(&self) -> &'static str {
        let port = self.remote_port.unwrap_or(self.local_port);
        match port {
            21 => "FTP",
            22 => "SSH",
            23 => "Telnet",
            25 => "SMTP",
            53 => "DNS",
            80 => "HTTP",
            110 => "POP3",
            143 => "IMAP",
            443 => "HTTPS",
            445 => "SMB",
            993 => "IMAPS",
            995 => "POP3S",
            1433 => "MSSQL",
            1521 => "Oracle",
            3306 => "MySQL",
            3389 => "RDP",
            5432 => "PgSQL",
            5900 => "VNC",
            6379 => "Redis",
            8080 => "HTTP-Alt",
            8443 => "HTTPS-Alt",
            27017 => "MongoDB",
            _ => "",
        }
    }
}

// ─── Win32 FFI structs ───────────────────────────────────────────────────────

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

// ─── FFI declarations ────────────────────────────────────────────────────────

#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(
        pTcpTable: *mut u8,
        pdwSize: *mut u32,
        bOrder: i32,
        ulAf: u32,
        TableClass: u32,
        Reserved: u32,
    ) -> u32;

    fn GetExtendedUdpTable(
        pUdpTable: *mut u8,
        pdwSize: *mut u32,
        bOrder: i32,
        ulAf: u32,
        TableClass: u32,
        Reserved: u32,
    ) -> u32;
}

// ─── Helper: network byte order to host ──────────────────────────────────────

fn ntohs(port: u32) -> u16 {
    ((port & 0xFF) << 8 | (port >> 8) & 0xFF) as u16
}

// ─── Fetch all connections ───────────────────────────────────────────────────

/// Fetch all TCP and UDP connections with owning PIDs.
/// Process names are resolved using the provided PID→name map.
pub fn fetch_connections(pid_names: &std::collections::HashMap<u32, String>) -> Vec<NetConnection> {
    let mut conns = Vec::with_capacity(256);
    fetch_tcp4(&mut conns);
    fetch_tcp6(&mut conns);
    fetch_udp4(&mut conns);
    fetch_udp6(&mut conns);

    // Resolve process names from the PID→name map (reuse process data)
    for conn in &mut conns {
        if let Some(name) = pid_names.get(&conn.pid) {
            conn.process_name = name.clone();
        } else if conn.pid == 0 {
            conn.process_name = "System Idle".to_string();
        } else if conn.pid == 4 {
            conn.process_name = "System".to_string();
        } else {
            conn.process_name = format!("PID:{}", conn.pid);
        }
    }

    conns
}

fn fetch_tcp4(conns: &mut Vec<NetConnection>) {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(
            std::ptr::null_mut(), &mut size, 0, AF_INET,
            TCP_TABLE_OWNER_PID_ALL, 0,
        );
        if size == 0 { return; }

        let mut buf = vec![0u8; size as usize];
        let ret = GetExtendedTcpTable(
            buf.as_mut_ptr(), &mut size, 0, AF_INET,
            TCP_TABLE_OWNER_PID_ALL, 0,
        );
        if ret != 0 { return; }

        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        for row in rows {
            conns.push(NetConnection {
                proto: ConnProto::Tcp,
                local_addr: IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                local_port: ntohs(row.dwLocalPort),
                remote_addr: Some(IpAddr::V4(Ipv4Addr::from(row.dwRemoteAddr.to_ne_bytes()))),
                remote_port: Some(ntohs(row.dwRemotePort)),
                state: Some(TcpState::from_raw(row.dwState)),
                pid: row.dwOwningPid,
                process_name: String::new(),
            });
        }
    }
}

fn fetch_tcp6(conns: &mut Vec<NetConnection>) {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(
            std::ptr::null_mut(), &mut size, 0, AF_INET6,
            TCP_TABLE_OWNER_PID_ALL, 0,
        );
        if size == 0 { return; }

        let mut buf = vec![0u8; size as usize];
        let ret = GetExtendedTcpTable(
            buf.as_mut_ptr(), &mut size, 0, AF_INET6,
            TCP_TABLE_OWNER_PID_ALL, 0,
        );
        if ret != 0 { return; }

        let table = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        for row in rows {
            conns.push(NetConnection {
                proto: ConnProto::Tcp,
                local_addr: IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                local_port: ntohs(row.dwLocalPort),
                remote_addr: Some(IpAddr::V6(Ipv6Addr::from(row.ucRemoteAddr))),
                remote_port: Some(ntohs(row.dwRemotePort)),
                state: Some(TcpState::from_raw(row.dwState)),
                pid: row.dwOwningPid,
                process_name: String::new(),
            });
        }
    }
}

fn fetch_udp4(conns: &mut Vec<NetConnection>) {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedUdpTable(
            std::ptr::null_mut(), &mut size, 0, AF_INET,
            UDP_TABLE_OWNER_PID, 0,
        );
        if size == 0 { return; }

        let mut buf = vec![0u8; size as usize];
        let ret = GetExtendedUdpTable(
            buf.as_mut_ptr(), &mut size, 0, AF_INET,
            UDP_TABLE_OWNER_PID, 0,
        );
        if ret != 0 { return; }

        let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        for row in rows {
            conns.push(NetConnection {
                proto: ConnProto::Udp,
                local_addr: IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                local_port: ntohs(row.dwLocalPort),
                remote_addr: None,
                remote_port: None,
                state: None,
                pid: row.dwOwningPid,
                process_name: String::new(),
            });
        }
    }
}

fn fetch_udp6(conns: &mut Vec<NetConnection>) {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedUdpTable(
            std::ptr::null_mut(), &mut size, 0, AF_INET6,
            UDP_TABLE_OWNER_PID, 0,
        );
        if size == 0 { return; }

        let mut buf = vec![0u8; size as usize];
        let ret = GetExtendedUdpTable(
            buf.as_mut_ptr(), &mut size, 0, AF_INET6,
            UDP_TABLE_OWNER_PID, 0,
        );
        if ret != 0 { return; }

        let table = &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(
            table.table.as_ptr(), table.dwNumEntries as usize,
        );
        for row in rows {
            conns.push(NetConnection {
                proto: ConnProto::Udp,
                local_addr: IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                local_port: ntohs(row.dwLocalPort),
                remote_addr: None,
                remote_port: None,
                state: None,
                pid: row.dwOwningPid,
                process_name: String::new(),
            });
        }
    }
}
