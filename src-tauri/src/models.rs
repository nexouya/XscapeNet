use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    Hourly,
    Today,
    TwoDays,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectorMode {
    /// Kernel ETW (`Microsoft-Windows-Kernel-Network` + `DNS-Client`) active (Administrator)
    KernelEtw,
    /// Non-admin fallback: exact 64-bit `GetIfTable2` totals + active socket table
    Standard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// "oled_black" (default), "obsidian", "violet", "emerald"
    pub theme: String,
    pub hide_ip: bool,
    /// "subtle", "liquid", "ultra"
    pub glass_intensity: String,
    pub run_on_startup: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "oled_black".into(),
            hide_ip: false,
            glass_intensity: "liquid".into(),
            run_on_startup: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub is_up: bool,
    pub link_speed_mbps: u64,
    pub rx_rate_bps: u64,
    pub tx_rate_bps: u64,
    pub rx_today_bytes: u64,
    pub tx_today_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkIdentity {
    pub public_ipv4: String,
    pub country_code: String,
    pub country_name: String,
    pub isp_asn: String,
    pub latency_ms: Option<u32>,
    pub local_ip: String,
    pub gateway: String,
    pub dns_servers: Vec<String>,
    pub active_interface: String,
    pub active_interface_kind: String,
    pub active_tcp_sockets: u32,
    pub active_udp_sockets: u32,
    pub updated_at: i64,
}

impl Default for NetworkIdentity {
    fn default() -> Self {
        Self {
            public_ipv4: "Detecting...".into(),
            country_code: "--".into(),
            country_name: "Unknown".into(),
            isp_asn: "Resolving...".into(),
            latency_ms: None,
            local_ip: "—".into(),
            gateway: "—".into(),
            dns_servers: vec![],
            active_interface: "Detecting...".into(),
            active_interface_kind: "Other".into(),
            active_tcp_sockets: 0,
            active_udp_sockets: 0,
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointUsageRow {
    pub remote_host: String,
    pub protocol: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessUsageRow {
    pub process_name: String,
    pub process_path: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub total_bytes: u64,
    pub share_percent: f64,
    pub active_connections: u32,
    pub endpoints: Vec<EndpointUsageRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesPoint {
    pub timestamp: i64,
    pub label: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub peak_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTick {
    pub timestamp: i64,
    pub collector_mode: CollectorMode,
    pub rx_rate_bps: u64,
    pub tx_rate_bps: u64,
    pub today_rx_bytes: u64,
    pub today_tx_bytes: u64,
    pub today_total_bytes: u64,
    pub interfaces: Vec<InterfaceInfo>,
    pub identity: NetworkIdentity,
    pub top_processes_today: Vec<ProcessUsageRow>,
    pub sparkline_today: Vec<TimeSeriesPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsResponse {
    pub range: TimeRange,
    pub interface_id: String,
    pub collector_mode: CollectorMode,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub total_bytes: u64,
    pub peak_bps: u64,
    pub series: Vec<TimeSeriesPoint>,
    pub processes: Vec<ProcessUsageRow>,
}

#[derive(Debug, Clone)]
pub struct InterfaceCounterSample {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub is_up: bool,
    pub link_speed_mbps: u64,
    pub in_octets: u64,
    pub out_octets: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessTrafficDelta {
    pub process_name: String,
    pub process_path: String,
    pub remote_host: String,
    pub protocol: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub connections: u32,
}
