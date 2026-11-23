#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetExtendedTcpTable, GetExtendedUdpTable, GetIfTable2, MIB_IF_ROW2,
    MIB_IF_TABLE2, MIB_TCPTABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    UDP_TABLE_OWNER_PID,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::AF_INET;
use windows::Win32::System::Diagnostics::Etw::{
    ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW, CONTROLTRACE_HANDLE,
    EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD, EVENT_TRACE_CONTROL_STOP,
    EVENT_TRACE_LOGFILEW, EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE,
    PROCESS_TRACE_MODE_EVENT_RECORD, PROCESS_TRACE_MODE_REAL_TIME, TRACE_LEVEL_INFORMATION,
    WNODE_FLAG_TRACED_GUID,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::models::{CollectorMode, InterfaceCounterSample, ProcessTrafficDelta};

const SESSION_NAME: &str = "XscapeNetKernelSession";

// Microsoft-Windows-Kernel-Network {7dd42a49-5329-4832-8dfd-43d979153a88}
const KERNEL_NETWORK_GUID: GUID = GUID::from_u128(0x7dd42a49_5329_4832_8dfd_43d979153a88);
// Microsoft-Windows-DNS-Client {1c95126e-7eea-49a9-a3fe-a378b03ddb4d}
const DNS_CLIENT_GUID: GUID = GUID::from_u128(0x1c95126e_7eea_49a9_a3fe_a378b03ddb4d);

#[derive(Default)]
struct EtwAccumulator {
    /// Key: (pid, remote_ip_str, protocol_label) -> (rx_bytes, tx_bytes)
    buckets: HashMap<(u32, String, String), (u64, u64)>,
    /// IP -> Domain name cache populated from DNS-Client ETW events
    dns_cache: HashMap<String, String>,
}

static GLOBAL_ETW: OnceLock<Arc<Mutex<EtwAccumulator>>> = OnceLock::new();

fn etw_store() -> &'static Arc<Mutex<EtwAccumulator>> {
    GLOBAL_ETW.get_or_init(|| Arc::new(Mutex::new(EtwAccumulator::default())))
}

pub struct PlatformCollector {
    mode: CollectorMode,
    pid_cache: Mutex<HashMap<u32, (String, String)>>,
    _etw_running: Arc<AtomicBool>,
}

impl PlatformCollector {
    pub fn new() -> Self {
        let running = Arc::new(AtomicBool::new(false));
        let mode = if start_etw_background(running.clone()) {
            CollectorMode::KernelEtw
        } else {
            CollectorMode::Standard
        };

        Self {
            mode,
            pid_cache: Mutex::new(HashMap::new()),
            _etw_running: running,
        }
    }

    pub fn mode(&self) -> CollectorMode {
        self.mode
    }

    /// Reads 64-bit hardware & VPN/TUN interface byte counters via Win32 `GetIfTable2`.
    pub fn sample_interfaces(&self) -> Vec<InterfaceCounterSample> {
        let mut out = Vec::new();
        unsafe {
            let mut table_ptr: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table_ptr) != ERROR_SUCCESS || table_ptr.is_null() {
                return out;
            }
            let count = (*table_ptr).NumEntries as usize;
            let rows: &[MIB_IF_ROW2] =
                std::slice::from_raw_parts((*table_ptr).Table.as_ptr(), count);

            for row in rows {
                // Skip loopback (24) and interfaces that are not operational
                if row.Type == 24 || row.OperStatus != IfOperStatusUp {
                    continue;
                }
                let flags = row.InterfaceAndOperStatusFlags._bitfield;
                let is_hw = (flags & 0x01) != 0;
                let is_filter = (flags & 0x02) != 0;
                let is_tunnel = row.Type == 53 || row.Type == 131;

                // Exclude NDIS filter miniports to prevent double-counting
                if is_filter || (!is_hw && !is_tunnel) {
                    continue;
                }

                let alias = utf16_array_to_string(&row.Alias);
                let desc = utf16_array_to_string(&row.Description);
                let desc_lower = desc.to_lowercase();
                if desc_lower.contains("miniport")
                    || desc_lower.contains("qos packet")
                    || desc_lower.contains("wfp ")
                    || desc_lower.contains("kernel debug")
                {
                    continue;
                }

                let kind = if row.Type == 71 {
                    "Wi-Fi"
                } else if is_tunnel
                    || desc_lower.contains("wireguard")
                    || desc_lower.contains("wintun")
                    || desc_lower.contains("tailscale")
                    || desc_lower.contains("tap-")
                    || desc_lower.contains("vpn")
                {
                    "VPN"
                } else if row.Type == 6 {
                    "Ethernet"
                } else {
                    "Other"
                };

                let luid_val = row.InterfaceLuid.Value;
                let speed_mbps = row.ReceiveLinkSpeed.max(row.TransmitLinkSpeed) / 1_000_000;

                out.push(InterfaceCounterSample {
                    id: format!("if_{:x}", luid_val),
                    name: if alias.is_empty() { desc.clone() } else { alias },
                    description: desc,
                    kind: kind.to_string(),
                    is_up: true,
                    link_speed_mbps: speed_mbps,
                    in_octets: row.InOctets,
                    out_octets: row.OutOctets,
                });
            }

            FreeMibTable(table_ptr as *const _);
        }
        out
    }

    /// Drains accumulated ETW byte deltas (if elevated) + enriches with active TCP/UDP socket counts.
    pub fn drain_process_deltas(&self) -> (Vec<ProcessTrafficDelta>, u32, u32) {
        let (tcp_count, udp_count, active_sockets) = self.sample_active_sockets();
        let mut out = Vec::new();

        if self.mode == CollectorMode::KernelEtw {
            let mut store = etw_store().lock();
            let drained: Vec<_> = store.buckets.drain().collect();
            let dns_snapshot = store.dns_cache.clone();
            drop(store);

            for ((pid, remote_ip, protocol), (rx, tx)) in drained {
                let (proc_name, proc_path) = self.resolve_pid(pid);
                let host = dns_snapshot
                    .get(&remote_ip)
                    .cloned()
                    .unwrap_or_else(|| friendly_host_label(&remote_ip));
                let conns = active_sockets.get(&pid).copied().unwrap_or(1);
                out.push(ProcessTrafficDelta {
                    process_name: proc_name,
                    process_path: proc_path,
                    remote_host: host,
                    protocol,
                    rx_bytes: rx,
                    tx_bytes: tx,
                    connections: conns,
                });
            }
        } else {
            // Standard (non-admin) mode: record active sockets by process so the user still sees
            // which apps are talking to the network, even without ETW per-packet byte telemetry.
            for (pid, conns) in active_sockets {
                if conns == 0 {
                    continue;
                }
                let (proc_name, proc_path) = self.resolve_pid(pid);
                out.push(ProcessTrafficDelta {
                    process_name: proc_name,
                    process_path: proc_path,
                    remote_host: "active-sockets".into(),
                    protocol: "TCP/UDP".into(),
                    rx_bytes: 0,
                    tx_bytes: 0,
                    connections: conns,
                });
            }
        }

        (out, tcp_count, udp_count)
    }

    fn sample_active_sockets(&self) -> (u32, u32, HashMap<u32, u32>) {
        let mut tcp_count = 0u32;
        let mut udp_count = 0u32;
        let mut by_pid: HashMap<u32, u32> = HashMap::new();

        unsafe {
            // TCPv4 table with owning PIDs
            let mut size = 0u32;
            let _ = GetExtendedTcpTable(
                None,
                &mut size,
                false,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );
            if size > 0 {
                let mut buf = vec![0u8; size as usize];
                if GetExtendedTcpTable(
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut size,
                    false,
                    AF_INET.0 as u32,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                ) == 0
                {
                    let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                    let rows = std::slice::from_raw_parts(
                        table.table.as_ptr(),
                        table.dwNumEntries as usize,
                    );
                    for row in rows {
                        // State 5 = MIB_TCP_STATE_ESTAB
                        if row.dwState == 5 {
                            tcp_count += 1;
                            *by_pid.entry(row.dwOwningPid).or_insert(0) += 1;
                        }
                    }
                }
            }

            // UDPv4 table with owning PIDs
            let mut usize_buf = 0u32;
            let _ = GetExtendedUdpTable(
                None,
                &mut usize_buf,
                false,
                AF_INET.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );
            if usize_buf > 0 {
                let mut buf = vec![0u8; usize_buf as usize];
                if GetExtendedUdpTable(
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut usize_buf,
                    false,
                    AF_INET.0 as u32,
                    UDP_TABLE_OWNER_PID,
                    0,
                ) == 0
                {
                    let table = &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
                    let rows = std::slice::from_raw_parts(
                        table.table.as_ptr(),
                        table.dwNumEntries as usize,
                    );
                    udp_count = rows.len() as u32;
                    for row in rows {
                        *by_pid.entry(row.dwOwningPid).or_insert(0) += 1;
                    }
                }
            }
        }

        (tcp_count, udp_count, by_pid)
    }

    fn resolve_pid(&self, pid: u32) -> (String, String) {
        if pid == 0 {
            return ("System Idle".into(), String::new());
        }
        if pid == 4 {
            return ("System".into(), "ntoskrnl.exe".into());
        }
        {
            let cache = self.pid_cache.lock();
            if let Some(hit) = cache.get(&pid) {
                return hit.clone();
            }
        }

        let resolved = unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
            if let Ok(h) = handle {
                let mut buf = vec![0u16; 512];
                let mut len = buf.len() as u32;
                let ok = QueryFullProcessImageNameW(
                    h,
                    PROCESS_NAME_WIN32,
                    PWSTR(buf.as_mut_ptr()),
                    &mut len,
                )
                .is_ok();
                let _ = CloseHandle(h);
                if ok && len > 0 {
                    let full_path = String::from_utf16_lossy(&buf[..len as usize]);
                    let name = full_path
                        .rsplit('\\')
                        .next()
                        .unwrap_or(&full_path)
                        .to_string();
                    (name, full_path)
                } else {
                    (format!("pid-{}", pid), String::new())
                }
            } else {
                (format!("pid-{}", pid), String::new())
            }
        };

        let mut cache = self.pid_cache.lock();
        if cache.len() > 2048 {
            cache.clear();
        }
        cache.insert(pid, resolved.clone());
        resolved
    }
}

fn utf16_array_to_string(slice: &[u16]) -> String {
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end])
}

fn friendly_host_label(ip: &str) -> String {
    match ip {
        "1.1.1.1" | "1.0.0.1" => "cloudflare-dns.com".into(),
        "8.8.8.8" | "8.8.4.4" => "dns.google".into(),
        _ => ip.to_string(),
    }
}

fn protocol_label(is_udp: bool, dport: u16) -> String {
    match (is_udp, dport) {
        (true, 443) => "QUIC".into(),
        (true, 53) => "DNS".into(),
        (false, 443) => "HTTPS".into(),
        (false, 80) => "HTTP".into(),
        (false, 22) => "SSH".into(),
        (true, _) => "UDP".into(),
        (false, _) => "TCP".into(),
    }
}

/// Starts a real-time ETW consumer thread subscribing to Kernel-Network & DNS-Client providers.
/// Returns `true` if the session started (Administrator), or `false` on Access Denied (Standard mode).
fn start_etw_background(running: Arc<AtomicBool>) -> bool {
    let mut name_utf16: Vec<u16> = SESSION_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    let props_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + (name_utf16.len() * 2) + 64;
    let mut props_buf = vec![0u8; props_size];

    unsafe {
        let props = &mut *(props_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES);
        props.Wnode.BufferSize = props_size as u32;
        props.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        props.Wnode.ClientContext = 1; // QPC clock
        props.LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        props.LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;

        // Clean up any stale session with the same name first
        let _ = ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            PCWSTR(name_utf16.as_ptr()),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );

        let mut session_handle = CONTROLTRACE_HANDLE::default();
        let status = StartTraceW(
            &mut session_handle,
            PCWSTR(name_utf16.as_ptr()),
            props,
        );

        if status != ERROR_SUCCESS && status != ERROR_ALREADY_EXISTS {
            return false;
        }

        // Enable Microsoft-Windows-Kernel-Network (keywords: all network I/O)
        let st_net = EnableTraceEx2(
            session_handle,
            &KERNEL_NETWORK_GUID,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            TRACE_LEVEL_INFORMATION as u8,
            0xFFFF,
            0,
            0,
            None,
        );
        if st_net != ERROR_SUCCESS {
            let _ = ControlTraceW(session_handle, PCWSTR::null(), props, EVENT_TRACE_CONTROL_STOP);
            return false;
        }

        // Best-effort enable Microsoft-Windows-DNS-Client for IP -> Domain resolution
        let _ = EnableTraceEx2(
            session_handle,
            &DNS_CLIENT_GUID,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            TRACE_LEVEL_INFORMATION as u8,
            0xFFFFFFFF,
            0,
            0,
            None,
        );

        running.store(true, Ordering::SeqCst);

        std::thread::Builder::new()
            .name("xscapenet-etw".into())
            .spawn(move || {
                let mut logfile = EVENT_TRACE_LOGFILEW::default();
                logfile.LoggerName = PWSTR(name_utf16.as_mut_ptr());
                logfile.Anonymous1.ProcessTraceMode =
                    PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
                logfile.Anonymous2.EventRecordCallback = Some(etw_event_callback);

                let trace_handle = OpenTraceW(&mut logfile);
                if trace_handle.Value != u64::MAX {
                    let handles = [trace_handle];
                    let _ = ProcessTrace(&handles, None, None);
                }
                running.store(false, Ordering::SeqCst);
            })
            .ok();
    }

    true
}

unsafe extern "system" fn etw_event_callback(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    let rec = &*record;
    let provider = rec.EventHeader.ProviderId;
    let event_id = rec.EventHeader.EventDescriptor.Id;
    let data_len = rec.UserDataLength as usize;
    if rec.UserData.is_null() || data_len == 0 {
        return;
    }
    let data = std::slice::from_raw_parts(rec.UserData as *const u8, data_len);

    if provider == KERNEL_NETWORK_GUID {
        // Microsoft-Windows-Kernel-Network manifest layout:
        // Event 10: TcpSendIpv4, 11: TcpRecvIpv4, 42: UdpSendIpv4, 43: UdpRecvIpv4
        // Layout for IPv4 events: PID (u32 @ 0), size (u32 @ 4), daddr (u32 @ 8), saddr (u32 @ 12), dport (u16 BE @ 16), sport (u16 BE @ 18)
        let is_v4 = matches!(event_id, 10 | 11 | 42 | 43);
        let is_v6 = matches!(event_id, 26 | 27 | 58 | 59);
        if !is_v4 && !is_v6 {
            return;
        }
        let is_send = matches!(event_id, 10 | 26 | 42 | 58);
        let is_udp = matches!(event_id, 42 | 43 | 58 | 59);

        let (pid, size, remote_ip, dport) = if is_v4 && data.len() >= 20 {
            let pid = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u64;
            let ip = IpAddr::V4(Ipv4Addr::new(data[8], data[9], data[10], data[11]));
            let dport = u16::from_be_bytes([data[16], data[17]]);
            (pid, size, ip, dport)
        } else if is_v6 && data.len() >= 44 {
            let pid = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u64;
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&data[8..24]);
            let ip = IpAddr::V6(Ipv6Addr::from(octets));
            let dport = u16::from_be_bytes([data[40], data[41]]);
            (pid, size, ip, dport)
        } else {
            return;
        };

        if size == 0 || remote_ip.is_loopback() {
            return;
        }

        let proto = protocol_label(is_udp, dport);
        let mut store = etw_store().lock();
        let entry = store
            .buckets
            .entry((pid, remote_ip.to_string(), proto))
            .or_insert((0, 0));
        if is_send {
            entry.1 = entry.1.saturating_add(size);
        } else {
            entry.0 = entry.0.saturating_add(size);
        }
    } else if provider == DNS_CLIENT_GUID && (event_id == 3008 || event_id == 3020) {
        // Parse UTF-16LE QueryName + QueryResults from DNS-Client event payload
        if let Some((domain, ips)) = parse_dns_client_payload(data) {
            let mut store = etw_store().lock();
            if store.dns_cache.len() > 8192 {
                store.dns_cache.clear();
            }
            for ip in ips {
                store.dns_cache.insert(ip, domain.clone());
            }
        }
    }
}

/// Extracts UTF-16LE domain name and resolved IPv4/IPv6 addresses from DNS-Client ETW payloads.
fn parse_dns_client_payload(data: &[u8]) -> Option<(String, Vec<String>)> {
    if data.len() < 8 {
        return None;
    }
    let u16s: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let null_pos = u16s.iter().position(|&c| c == 0)?;
    let domain = String::from_utf16_lossy(&u16s[..null_pos])
        .trim_end_matches('.')
        .to_lowercase();
    if domain.is_empty() || !domain.contains('.') {
        return None;
    }
    let rest = String::from_utf16_lossy(&u16s[null_pos + 1..]);
    let mut ips = Vec::new();
    for token in rest.split(|c: char| c == ';' || c == '\0' || c.is_whitespace()) {
        let cleaned = token.trim_start_matches("type: 5 ").trim();
        if let Ok(ip) = cleaned.parse::<IpAddr>() {
            ips.push(ip.to_string());
        }
    }
    if ips.is_empty() {
        None
    } else {
        Some((domain, ips))
    }
}

/// Relaunches XscapeNet with Administrator privileges via Windows UAC (`runas`) to enable Kernel ETW.
pub fn relaunch_elevated() -> Result<(), String> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_w: Vec<u16> = exe
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let verb_w: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let res = ShellExecuteW(
            None,
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        if (res.0 as isize) <= 32 {
            return Err("UAC elevation was cancelled or failed.".into());
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _keep_win32_error_import(_: WIN32_ERROR) {}
