/// Per-core CPU usage info
#[derive(Debug, Clone, Default)]
pub struct CpuCore {
    pub id: usize,
    pub usage_percent: f32,
    pub frequency_mhz: u64,
}

/// Aggregate CPU information
#[derive(Debug, Clone, Default)]
pub struct CpuInfo {
    pub cores: Vec<CpuCore>,
    pub total_usage: f32,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub brand: String,
}

impl CpuInfo {
    pub fn new() -> Self {
        Self::default()
    }
}
