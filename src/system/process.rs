/// Sort field options for the process table — matches htop's default columns:
/// PID USER PRI NI VIRT RES SHR S CPU% MEM% TIME+ Command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSortField {
    Pid,
    Ppid,
    User,
    Priority,
    Nice,
    VirtMem,
    ResMem,
    SharedMem,
    Status,
    Cpu,
    Mem,
    Time,
    Threads,
    Command,
    IoReadRate,
    IoWriteRate,
    IoRate,
}

impl ProcessSortField {
    /// Short header label (displayed in the column header row)
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Ppid => "PPID",
            Self::User => "USER",
            Self::Priority => "PRI",
            Self::Nice => "NI",
            Self::VirtMem => "VIRT",
            Self::ResMem => "RES",
            Self::SharedMem => "SHR",
            Self::Status => "S",
            Self::Cpu => "CPU%",
            Self::Mem => "MEM%",
            Self::Time => "TIME+",
            Self::Threads => "THR",
            Self::Command => "Command",
            Self::IoReadRate => "DISK READ",
            Self::IoWriteRate => "DISK WRITE",
            Self::IoRate => "DISK R/W",
        }
    }

    /// Long label for sort menu
    pub fn long_label(&self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Ppid => "PPID",
            Self::User => "USER",
            Self::Priority => "PRIORITY",
            Self::Nice => "NICE",
            Self::VirtMem => "M_VIRT",
            Self::ResMem => "M_RESIDENT",
            Self::SharedMem => "M_SHARE",
            Self::Status => "STATE",
            Self::Cpu => "PERCENT_CPU",
            Self::Mem => "PERCENT_MEM",
            Self::Time => "TIME+",
            Self::Threads => "THREADS",
            Self::Command => "Command",
            Self::IoReadRate => "IO_READ_RATE",
            Self::IoWriteRate => "IO_WRITE_RATE",
            Self::IoRate => "IO_RATE",
        }
    }

    /// All fields in htop default column order
    pub fn all() -> &'static [ProcessSortField] {
        &[
            Self::Pid,
            Self::Ppid,
            Self::User,
            Self::Priority,
            Self::Nice,
            Self::VirtMem,
            Self::ResMem,
            Self::SharedMem,
            Self::Status,
            Self::Cpu,
            Self::Mem,
            Self::Time,
            Self::Threads,
            Self::IoReadRate,
            Self::IoWriteRate,
            Self::IoRate,
            Self::Command,
        ]
    }

    /// Get index in `all()` list
    pub fn index(&self) -> usize {
        Self::all().iter().position(|f| f == self).unwrap_or(0)
    }
}

/// Process status (Windows mapped to htop-like labels)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessStatus {
    Running,
    Sleeping,
    DiskSleep,
    Stopped,
    Zombie,
    Unknown,
}

impl ProcessStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Running => "R",
            Self::Sleeping => "S",
            Self::DiskSleep => "D",
            Self::Stopped => "T",
            Self::Zombie => "Z",
            Self::Unknown => "?",
        }
    }
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

/// Information about a single process
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub command: String,
    pub user: String,
    pub status: ProcessStatus,
    pub priority: i32,
    pub nice: i32,
    pub virtual_mem: u64,    // bytes
    pub resident_mem: u64,   // bytes
    pub shared_mem: u64,     // bytes
    pub cpu_usage: f32,      // percentage
    pub mem_usage: f32,      // percentage
    pub run_time: u64,       // seconds
    pub threads: u32,
    // I/O statistics
    pub io_read_rate: f64,   // bytes/second
    pub io_write_rate: f64,  // bytes/second
    // For tree view
    pub depth: usize,
    pub is_last_child: bool,
    // Tagging
    pub tagged: bool,
}

impl ProcessInfo {
    /// Format run time as h:MM:SS or M:SS.cc (hundredths) — matches htop TIME+
    pub fn format_time(&self) -> String {
        let total = self.run_time;
        let hours = total / 3600;
        let minutes = (total % 3600) / 60;
        let seconds = total % 60;
        let centiseconds = 0u64; // We don't have sub-second from sysinfo

        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{}:{:02}.{:02}", minutes, seconds, centiseconds)
        }
    }
}
