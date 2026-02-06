/// Memory usage information
#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub total_mem: u64,      // bytes
    pub used_mem: u64,       // bytes
    pub free_mem: u64,       // bytes
    pub cached_mem: u64,     // bytes
    pub buffered_mem: u64,   // bytes (not separated on Windows)
    pub total_swap: u64,     // bytes
    pub used_swap: u64,      // bytes
    pub free_swap: u64,      // bytes
}

impl MemoryInfo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Memory usage as percentage
    pub fn mem_percent(&self) -> f64 {
        if self.total_mem == 0 {
            0.0
        } else {
            (self.used_mem as f64 / self.total_mem as f64) * 100.0
        }
    }

    /// Swap usage as percentage
    pub fn swap_percent(&self) -> f64 {
        if self.total_swap == 0 {
            0.0
        } else {
            (self.used_swap as f64 / self.total_swap as f64) * 100.0
        }
    }
}

/// Format bytes to human-readable string (KiB, MiB, GiB)
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.1}T", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.1}G", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.0}M", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.0}K", bytes as f64 / KIB as f64)
    } else {
        format!("{}B", bytes)
    }
}
