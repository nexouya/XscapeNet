use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::ip_info;
use crate::models::{
    AnalyticsResponse, AppSettings, CollectorMode, InterfaceInfo, LiveTick, NetworkIdentity,
    TimeRange,
};
use crate::startup;
use crate::storage::Storage;
use crate::tray;

#[cfg(target_os = "windows")]
use crate::collector_win::PlatformCollector;

#[cfg(not(target_os = "windows"))]
use crate::collector_fallback::PlatformCollector;

pub struct AppState {
    pub storage: Arc<Storage>,
    pub collector: Arc<PlatformCollector>,
    pub identity: Arc<RwLock<NetworkIdentity>>,
    pub last_tick: Arc<RwLock<Option<LiveTick>>>,
    pub settings: Arc<RwLock<AppSettings>>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let storage = Arc::new(Storage::open().map_err(|e| e.to_string())?);
        let collector = Arc::new(PlatformCollector::new());
        let identity = Arc::new(RwLock::new(NetworkIdentity::default()));
        let last_tick = Arc::new(RwLock::new(None));
        let mut loaded = storage.load_settings();
        loaded.run_on_startup = startup::is_run_on_startup_enabled();
        let settings = Arc::new(RwLock::new(loaded));
        Ok(Self {
            storage,
            collector,
            identity,
            last_tick,
            settings,
        })
    }
}

pub fn format_bytes_short(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_rate_short(bps: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bps as f64;
    if b >= MB {
        format!("{:.1} MB/s", b / MB)
    } else if b >= KB {
        format!("{:.0} KB/s", b / KB)
    } else {
        format!("{} B/s", bps)
    }
}

pub fn start_telemetry_loop<R: Runtime>(app: AppHandle<R>, state: Arc<AppState>) {
    let id_ref = state.identity.clone();
    tauri::async_runtime::spawn(async move {
        let current = id_ref.read().clone();
        let refreshed = ip_info::refresh_public_identity(&current).await;
        *id_ref.write() = refreshed;
    });

    tauri::async_runtime::spawn(async move {
        let mut prev_counters: HashMap<String, (u64, u64)> = HashMap::new();
        let mut tick_count: u64 = 0;
        let mut last_primary_if = String::new();
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            tick_count += 1;
            let now_ts = Local::now().timestamp();

            let samples = state.collector.sample_interfaces();
            let mut deltas: Vec<(String, u64, u64)> = Vec::new();
            let mut total_rx_rate: u64 = 0;
            let mut total_tx_rate: u64 = 0;
            let mut primary_if_id = "all".to_string();
            let mut primary_if_rate = 0u64;

            for s in &samples {
                let (rx_delta, tx_delta) = if let Some(&(prev_in, prev_out)) = prev_counters.get(&s.id)
                {
                    let rx = s.in_octets.saturating_sub(prev_in);
                    let tx = s.out_octets.saturating_sub(prev_out);
                    if rx > 20_000_000_000 || tx > 20_000_000_000 {
                        (0, 0)
                    } else {
                        (rx, tx)
                    }
                } else {
                    (0, 0)
                };
                prev_counters.insert(s.id.clone(), (s.in_octets, s.out_octets));
                total_rx_rate = total_rx_rate.saturating_add(rx_delta);
                total_tx_rate = total_tx_rate.saturating_add(tx_delta);
                if rx_delta + tx_delta >= primary_if_rate {
                    primary_if_rate = rx_delta + tx_delta;
                    primary_if_id = s.id.clone();
                }
                if rx_delta > 0 || tx_delta > 0 {
                    deltas.push((s.id.clone(), rx_delta, tx_delta));
                }
            }

            let _ = state.storage.record_interface_deltas(now_ts, &deltas);

            let (proc_deltas, tcp_count, udp_count) = state.collector.drain_process_deltas();
            if !proc_deltas.is_empty() {
                let _ = state
                    .storage
                    .record_process_deltas(now_ts, &primary_if_id, &proc_deltas);
            }

            // Periodic WAL checkpoint every 60s so data is always safely persisted on disk
            if tick_count % 60 == 0 {
                state.storage.checkpoint_wal();
            }

            if tick_count % 300 == 0
                || (!primary_if_id.is_empty() && primary_if_id != "all" && primary_if_id != last_primary_if)
            {
                last_primary_if = primary_if_id.clone();
                let id_clone = state.identity.clone();
                tauri::async_runtime::spawn(async move {
                    let cur = id_clone.read().clone();
                    let next = ip_info::refresh_public_identity(&cur).await;
                    *id_clone.write() = next;
                });
            }

            if tick_count % 3600 == 0 {
                let _ = state.storage.prune_old_buckets(now_ts);
            }

            let identity_snapshot = {
                let mut id_w = state.identity.write();
                id_w.active_tcp_sockets = tcp_count;
                id_w.active_udp_sockets = udp_count;
                id_w.clone()
            };

            let today_by_if = state
                .storage
                .get_today_interface_totals()
                .unwrap_or_default();

            let mut today_rx_total = 0u64;
            let mut today_tx_total = 0u64;
            for (rx, tx) in today_by_if.values() {
                today_rx_total = today_rx_total.saturating_add(*rx);
                today_tx_total = today_tx_total.saturating_add(*tx);
            }

            let interfaces: Vec<InterfaceInfo> = samples
                .iter()
                .map(|s| {
                    let (rx_t, tx_t) = today_by_if.get(&s.id).copied().unwrap_or((0, 0));
                    let (rx_r, tx_r) = deltas
                        .iter()
                        .find(|d| d.0 == s.id)
                        .map(|d| (d.1, d.2))
                        .unwrap_or((0, 0));
                    InterfaceInfo {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        description: s.description.clone(),
                        kind: s.kind.clone(),
                        is_up: s.is_up,
                        link_speed_mbps: s.link_speed_mbps,
                        rx_rate_bps: rx_r,
                        tx_rate_bps: tx_r,
                        rx_today_bytes: rx_t,
                        tx_today_bytes: tx_t,
                    }
                })
                .collect();

            let today_analytics = state
                .storage
                .query_analytics(TimeRange::Today, "all", state.collector.mode())
                .ok();

            let (sparkline_today, top_processes_today) = match today_analytics {
                Some(a) => (a.series, a.processes.into_iter().take(5).collect()),
                None => (Vec::new(), Vec::new()),
            };

            let tick = LiveTick {
                timestamp: now_ts,
                collector_mode: state.collector.mode(),
                rx_rate_bps: total_rx_rate,
                tx_rate_bps: total_tx_rate,
                today_rx_bytes: today_rx_total,
                today_tx_bytes: today_tx_total,
                today_total_bytes: today_rx_total.saturating_add(today_tx_total),
                interfaces,
                identity: identity_snapshot.clone(),
                top_processes_today,
                sparkline_today,
            };

            let hide_ip = state.settings.read().hide_ip;
            let ip_display = if hide_ip {
                "Hidden".to_string()
            } else {
                format!(
                    "{} ({})",
                    identity_snapshot.public_ipv4, identity_snapshot.country_code
                )
            };
            let tooltip = format!(
                "XscapeNet\n↓ {} · ↑ {}\nToday: {} (↓ {} / ↑ {})\nIP: {}",
                format_rate_short(tick.rx_rate_bps),
                format_rate_short(tick.tx_rate_bps),
                format_bytes_short(tick.today_total_bytes),
                format_bytes_short(tick.today_rx_bytes),
                format_bytes_short(tick.today_tx_bytes),
                ip_display,
            );
            tray::update_tray_tooltip(&app, &tooltip);

            *state.last_tick.write() = Some(tick.clone());
            let _ = app.emit("xscapenet://tick", &tick);
        }
    });
}

pub async fn trigger_ip_refresh<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        let cur = state.identity.read().clone();
        let next = ip_info::refresh_public_identity(&cur).await;
        *state.identity.write() = next;
    }
}

#[tauri::command]
pub fn get_live_snapshot(state: State<'_, Arc<AppState>>) -> Option<LiveTick> {
    state.last_tick.read().clone()
}

#[tauri::command]
pub fn get_analytics(
    state: State<'_, Arc<AppState>>,
    range: TimeRange,
    interface_id: String,
) -> Result<AnalyticsResponse, String> {
    state
        .storage
        .query_analytics(range, &interface_id, state.collector.mode())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_ip(state: State<'_, Arc<AppState>>) -> Result<NetworkIdentity, String> {
    let cur = state.identity.read().clone();
    let next = ip_info::refresh_public_identity(&cur).await;
    *state.identity.write() = next.clone();
    Ok(next)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> AppSettings {
    let mut s = state.settings.read().clone();
    s.run_on_startup = startup::is_run_on_startup_enabled();
    s
}

#[tauri::command]
pub fn update_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<AppState>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    startup::set_run_on_startup(settings.run_on_startup)?;
    state
        .storage
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;
    state.storage.checkpoint_wal();
    *state.settings.write() = settings.clone();
    let _ = app.emit("xscapenet://settings", &settings);
    Ok(settings)
}

#[tauri::command]
pub fn open_advanced_window<R: Runtime>(app: AppHandle<R>) {
    tray::open_advanced(&app);
}

#[tauri::command]
pub fn hide_popover<R: Runtime>(app: AppHandle<R>) {
    if let Some(pop) = app.get_webview_window("popover") {
        let _ = pop.hide();
    }
}

#[tauri::command]
pub fn clear_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .storage
        .clear_all_history()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_collector_mode(state: State<'_, Arc<AppState>>) -> CollectorMode {
    state.collector.mode()
}

#[tauri::command]
pub fn relaunch_as_admin<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(state) = app.try_state::<Arc<AppState>>() {
            state.storage.checkpoint_wal();
        }
        crate::collector_win::relaunch_elevated()?;
        app.exit(0);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Elevation is only applicable on Windows.".into())
    }
}

#[tauri::command]
pub fn get_blocked_apps(state: State<'_, Arc<AppState>>) -> Vec<String> {
    state.storage.list_blocked_apps()
}

#[tauri::command]
pub fn toggle_app_block(
    state: State<'_, Arc<AppState>>,
    process_name: String,
    process_path: String,
    block: bool,
) -> Result<Vec<String>, String> {
    crate::firewall::set_app_blocked(&process_name, &process_path, block)?;
    state
        .storage
        .set_blocked_app_record(&process_name, &process_path, block)
        .map_err(|e| e.to_string())?;
    state.storage.checkpoint_wal();
    Ok(state.storage.list_blocked_apps())
}
