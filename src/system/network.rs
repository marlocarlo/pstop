/// Network information (system-wide, per-interface aggregated)
#[derive(Debug, Clone, Default)]
pub struct NetworkInfo {
    /// Total bytes received per second across all interfaces
    pub rx_bytes_per_sec: f64,
    /// Total bytes transmitted per second across all interfaces
    pub tx_bytes_per_sec: f64,
    /// Total received since boot (bytes)
    pub total_rx: u64,
    /// Total transmitted since boot (bytes)
    pub total_tx: u64,
}
