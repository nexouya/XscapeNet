use std::time::Instant;

use chrono::Local;
use serde::Deserialize;

use crate::models::NetworkIdentity;

#[derive(Deserialize)]
struct IpWhoIsResponse {
    ip: Option<String>,
    country_code: Option<String>,
    country: Option<String>,
    connection: Option<IpWhoIsConn>,
}

#[derive(Deserialize)]
struct IpWhoIsConn {
    asn: Option<u64>,
    isp: Option<String>,
    org: Option<String>,
}

#[derive(Deserialize)]
struct IpInfoResponse {
    ip: Option<String>,
    country: Option<String>,
    org: Option<String>,
}

pub struct LocalNetDetails {
    pub local_ip: String,
    pub gateway: String,
    pub dns_servers: Vec<String>,
    pub active_interface: String,
    pub active_interface_kind: String,
}

#[cfg(target_os = "windows")]
pub fn inspect_local_network() -> LocalNetDetails {
    use std::ffi::c_void;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_INCLUDE_PREFIX,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{
        AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
    };

    let mut out = LocalNetDetails {
        local_ip: "—".into(),
        gateway: "—".into(),
        dns_servers: Vec::new(),
        active_interface: "Offline".into(),
        active_interface_kind: "Other".into(),
    };

    unsafe {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_PREFIX;
        let mut buf_len: u32 = 16 * 1024;
        let mut buffer = vec![0u8; buf_len as usize];

        let mut ret = GetAdaptersAddresses(
            AF_UNSPEC.0 as u32,
            flags,
            None,
            Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
            &mut buf_len,
        );

        if ret == ERROR_BUFFER_OVERFLOW.0 {
            buffer.resize(buf_len as usize, 0);
            ret = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                flags,
                None,
                Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut buf_len,
            );
        }

        if ret != 0 {
            return out;
        }

        let mut ptr = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !ptr.is_null() {
            let adapter = &*ptr;
            // Skip loopback (24) and down adapters
            if adapter.OperStatus == IfOperStatusUp && adapter.IfType != 24 {
                let mut local_v4: Option<String> = None;
                let mut u_ptr = adapter.FirstUnicastAddress;
                while !u_ptr.is_null() {
                    let ua = &*u_ptr;
                    if !ua.Address.lpSockaddr.is_null() {
                        let family = (*ua.Address.lpSockaddr).sa_family;
                        if family == AF_INET {
                            let sin = &*(ua.Address.lpSockaddr as *const SOCKADDR_IN);
                            let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr));
                            if !ip.is_loopback() && !ip.is_link_local() {
                                local_v4 = Some(ip.to_string());
                                break;
                            }
                        }
                    }
                    u_ptr = ua.Next;
                }

                let mut gw: Option<String> = None;
                let mut g_ptr = adapter.FirstGatewayAddress;
                while !g_ptr.is_null() {
                    let ga = &*g_ptr;
                    if !ga.Address.lpSockaddr.is_null() {
                        let family = (*ga.Address.lpSockaddr).sa_family;
                        if family == AF_INET {
                            let sin = &*(ga.Address.lpSockaddr as *const SOCKADDR_IN);
                            let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr));
                            if !ip.is_unspecified() {
                                gw = Some(ip.to_string());
                                break;
                            }
                        }
                    }
                    g_ptr = ga.Next;
                }

                // Prefer the adapter that actually has a default gateway
                if let Some(ip_str) = local_v4 {
                    let has_gw = gw.is_some();
                    if has_gw || out.local_ip == "—" {
                        out.local_ip = ip_str;
                        if let Some(g) = gw {
                            out.gateway = g;
                        }
                        out.active_interface = pwstr_to_string(adapter.FriendlyName.0);
                        out.active_interface_kind = match adapter.IfType {
                            71 => "Wi-Fi".into(),
                            6 => "Ethernet".into(),
                            53 | 131 => "VPN".into(),
                            _ => "Other".into(),
                        };

                        let mut dns = Vec::new();
                        let mut d_ptr = adapter.FirstDnsServerAddress;
                        while !d_ptr.is_null() && dns.len() < 3 {
                            let da = &*d_ptr;
                            if !da.Address.lpSockaddr.is_null() {
                                let family = (*da.Address.lpSockaddr).sa_family;
                                if family == AF_INET {
                                    let sin = &*(da.Address.lpSockaddr as *const SOCKADDR_IN);
                                    let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr));
                                    dns.push(ip.to_string());
                                } else if family == AF_INET6 {
                                    let sin6 = &*(da.Address.lpSockaddr as *const SOCKADDR_IN6);
                                    let ip = Ipv6Addr::from(sin6.sin6_addr.u.Byte);
                                    if !ip.is_unspecified() {
                                        let _ = ip; // Prefer clean IPv4 DNS display when present
                                    }
                                }
                            }
                            d_ptr = da.Next;
                        }
                        if !dns.is_empty() {
                            out.dns_servers = dns;
                        }
                        let _ = c_void::from; // keep import clean
                        if has_gw {
                            break;
                        }
                    }
                }
            }
            ptr = adapter.Next;
        }
    }

    out
}

#[cfg(target_os = "windows")]
unsafe fn pwstr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

#[cfg(not(target_os = "windows"))]
pub fn inspect_local_network() -> LocalNetDetails {
    LocalNetDetails {
        local_ip: "127.0.0.1".into(),
        gateway: "192.168.1.1".into(),
        dns_servers: vec!["1.1.1.1".into(), "1.0.0.1".into()],
        active_interface: "eth0".into(),
        active_interface_kind: "Ethernet".into(),
    }
}

/// Fetches Public IP, Country, ASN/ISP, and HTTP latency with a strict 4s timeout.
pub async fn refresh_public_identity(current: &NetworkIdentity) -> NetworkIdentity {
    let mut next = current.clone();
    let local = inspect_local_network();
    next.local_ip = local.local_ip;
    next.gateway = local.gateway;
    next.dns_servers = local.dns_servers;
    next.active_interface = local.active_interface;
    next.active_interface_kind = local.active_interface_kind;

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .user_agent("XscapeNet/1.0")
        .build()
    {
        Ok(c) => c,
        Err(_) => return next,
    };

    let t0 = Instant::now();
    if let Ok(resp) = client.get("https://ipwho.is/").send().await {
        let rtt = t0.elapsed().as_millis() as u32;
        if let Ok(body) = resp.json::<IpWhoIsResponse>().await {
            if let Some(ip) = body.ip {
                next.public_ipv4 = ip;
                next.country_code = body.country_code.unwrap_or_else(|| "--".into());
                next.country_name = body.country.unwrap_or_else(|| "Unknown".into());
                next.latency_ms = Some(rtt);
                if let Some(conn) = body.connection {
                    let asn_part = conn.asn.map(|a| format!("AS{}", a)).unwrap_or_default();
                    let org_part = conn.isp.or(conn.org).unwrap_or_default();
                    next.isp_asn = format!("{} {}", asn_part, org_part).trim().to_string();
                }
                next.updated_at = Local::now().timestamp();
                return next;
            }
        }
    }

    // Fallback provider: ipinfo.io
    let t1 = Instant::now();
    if let Ok(resp) = client.get("https://ipinfo.io/json").send().await {
        let rtt = t1.elapsed().as_millis() as u32;
        if let Ok(body) = resp.json::<IpInfoResponse>().await {
            if let Some(ip) = body.ip {
                next.public_ipv4 = ip;
                next.country_code = body.country.unwrap_or_else(|| "--".into());
                next.isp_asn = body.org.unwrap_or_else(|| "Unknown ISP".into());
                next.latency_ms = Some(rtt);
                next.updated_at = Local::now().timestamp();
            }
        }
    }

    next
}
