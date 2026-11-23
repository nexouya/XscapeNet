#![cfg(not(target_os = "windows"))]

use crate::models::{CollectorMode, InterfaceCounterSample, ProcessTrafficDelta};

pub struct PlatformCollector;

impl PlatformCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn mode(&self) -> CollectorMode {
        CollectorMode::Standard
    }

    pub fn sample_interfaces(&self) -> Vec<InterfaceCounterSample> {
        let mut out = Vec::new();
        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                let name = parts[0].trim();
                if name == "lo" {
                    continue;
                }
                let nums: Vec<u64> = parts[1]
                    .split_whitespace()
                    .filter_map(|s| s.parse::<u64>().ok())
                    .collect();
                if nums.len() >= 10 {
                    let kind = if name.starts_with("wl") {
                        "Wi-Fi"
                    } else if name.starts_with("en") || name.starts_with("eth") {
                        "Ethernet"
                    } else if name.starts_with("tun") || name.starts_with("wg") {
                        "VPN"
                    } else {
                        "Other"
                    };
                    out.push(InterfaceCounterSample {
                        id: name.to_string(),
                        name: name.to_string(),
                        description: name.to_string(),
                        kind: kind.to_string(),
                        is_up: true,
                        link_speed_mbps: 1000,
                        in_octets: nums[0],
                        out_octets: nums[8],
                    });
                }
            }
        }
        out
    }

    pub fn drain_process_deltas(&self) -> (Vec<ProcessTrafficDelta>, u32, u32) {
        (Vec::new(), 0, 0)
    }
}
