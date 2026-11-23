import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  AnalyticsResponse,
  AppSettings,
  LiveTick,
  NetworkIdentity,
  TimeRange,
} from "./types";

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "oled_black",
  hideIp: false,
  glassIntensity: "liquid",
  runOnStartup: false,
};

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function fetchLiveSnapshot(): Promise<LiveTick | null> {
  if (!isTauriRuntime()) return buildPreviewLiveTick();
  return invoke<LiveTick | null>("get_live_snapshot");
}

export async function fetchAnalytics(
  range: TimeRange,
  interfaceId: string
): Promise<AnalyticsResponse> {
  if (!isTauriRuntime()) return buildPreviewAnalytics(range, interfaceId);
  return invoke<AnalyticsResponse>("get_analytics", { range, interfaceId });
}

export async function fetchSettings(): Promise<AppSettings> {
  if (!isTauriRuntime()) return DEFAULT_SETTINGS;
  return invoke<AppSettings>("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<AppSettings> {
  if (!isTauriRuntime()) return settings;
  return invoke<AppSettings>("update_settings", { settings });
}


export async function fetchBlockedApps(): Promise<string[]> {
  if (!isTauriRuntime()) return [];
  return invoke<string[]>("get_blocked_apps");
}

export async function toggleAppBlock(
  processName: string,
  processPath: string,
  block: boolean
): Promise<string[]> {
  if (!isTauriRuntime()) return block ? [processName] : [];
  return invoke<string[]>("toggle_app_block", { processName, processPath, block });
}

export async function refreshPublicIp(): Promise<NetworkIdentity> {
  if (!isTauriRuntime()) return buildPreviewLiveTick().identity;
  return invoke<NetworkIdentity>("refresh_ip");
}

export async function openAdvancedWindow(): Promise<void> {
  if (!isTauriRuntime()) {
    window.location.search = "?view=advanced";
    return;
  }
  await invoke("open_advanced_window");
}

export async function hidePopoverWindow(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("hide_popover");
}

export async function clearUsageHistory(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("clear_history");
}

export async function relaunchAsAdmin(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("relaunch_as_admin");
}

export async function subscribeLiveTicks(
  onTick: (tick: LiveTick) => void
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    const timer = window.setInterval(() => onTick(buildPreviewLiveTick()), 1000);
    return () => window.clearInterval(timer);
  }
  return listen<LiveTick>("xscapenet://tick", (event) => {
    onTick(event.payload);
  });
}

export async function subscribeSettings(
  onSettings: (settings: AppSettings) => void
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<AppSettings>("xscapenet://settings", (event) => {
    onSettings(event.payload);
  });
}

export async function startWindowDrag(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().startDragging();
}

export async function minimizeCurrentWindow(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().minimize();
}

export async function toggleMaximizeCurrentWindow(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().toggleMaximize();
}

export async function closeCurrentWindow(): Promise<void> {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().close();
}

function buildPreviewLiveTick(): LiveTick {
  const now = Math.floor(Date.now() / 1000);
  const sparkline = Array.from({ length: 16 }, (_, i) => {
    const rx = Math.round((120 + Math.sin(i * 0.7) * 90 + (i === 13 ? 420 : 0)) * 1024 * 1024);
    const tx = Math.round((22 + Math.cos(i * 0.5) * 14) * 1024 * 1024);
    return {
      timestamp: now - (15 - i) * 3600,
      label: `${String((8 + i) % 24).padStart(2, "0")}:00`,
      rxBytes: rx,
      txBytes: tx,
      peakBps: Math.round((rx / 3600) * 14),
    };
  });

  const processes = [
    {
      processName: "chrome.exe",
      processPath: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
      rxBytes: 2_158_000_000,
      txBytes: 310_000_000,
      totalBytes: 2_468_000_000,
      sharePercent: 50.1,
      activeConnections: 38,
      endpoints: [
        { remoteHost: "youtube.com", protocol: "QUIC", rxBytes: 1_739_000_000, txBytes: 94_000_000, totalBytes: 1_833_000_000 },
        { remoteHost: "github.com", protocol: "HTTPS", rxBytes: 356_000_000, txBytes: 184_000_000, totalBytes: 540_000_000 },
      ],
    },
    {
      processName: "tailscaled.exe",
      processPath: "C:\\Program Files\\Tailscale\\tailscaled.exe",
      rxBytes: 1_267_000_000,
      txBytes: 142_000_000,
      totalBytes: 1_409_000_000,
      sharePercent: 28.6,
      activeConnections: 12,
      endpoints: [
        { remoteHost: "derp.tailscale.com", protocol: "UDP", rxBytes: 1_267_000_000, txBytes: 142_000_000, totalBytes: 1_409_000_000 },
      ],
    },
    {
      processName: "Telegram.exe",
      processPath: "C:\\Users\\Pouya\\AppData\\Roaming\\Telegram Desktop\\Telegram.exe",
      rxBytes: 432_000_000,
      txBytes: 198_000_000,
      totalBytes: 630_000_000,
      sharePercent: 12.8,
      activeConnections: 9,
      endpoints: [
        { remoteHost: "149.154.167.91", protocol: "HTTPS", rxBytes: 432_000_000, txBytes: 198_000_000, totalBytes: 630_000_000 },
      ],
    },
  ];

  return {
    timestamp: now,
    collectorMode: "kernel_etw",
    rxRateBps: 12_980_000,
    txRateBps: 1_890_000,
    todayRxBytes: 4_498_000_000,
    todayTxBytes: 664_000_000,
    todayTotalBytes: 5_162_000_000,
    interfaces: [
      {
        id: "if_wifi",
        name: "Wi-Fi 6E",
        description: "Intel Wi-Fi 6E AX211",
        kind: "Wi-Fi",
        isUp: true,
        linkSpeedMbps: 2402,
        rxRateBps: 12_980_000,
        txRateBps: 1_890_000,
        rxTodayBytes: 4_498_000_000,
        txTodayBytes: 664_000_000,
      },
    ],
    identity: {
      publicIpv4: "185.199.108.153",
      countryCode: "DE",
      countryName: "Germany",
      ispAsn: "AS13335 Cloudflare",
      latencyMs: 24,
      localIp: "192.168.1.14",
      gateway: "192.168.1.1",
      dnsServers: ["1.1.1.1", "1.0.0.1"],
      activeInterface: "Wi-Fi 6E",
      activeInterfaceKind: "Wi-Fi",
      activeTcpSockets: 84,
      activeUdpSockets: 32,
      updatedAt: now,
    },
    topProcessesToday: processes,
    sparklineToday: sparkline,
  };
}

function buildPreviewAnalytics(range: TimeRange, interfaceId: string): AnalyticsResponse {
  const tick = buildPreviewLiveTick();
  return {
    range,
    interfaceId,
    collectorMode: "kernel_etw",
    totalRxBytes: tick.todayRxBytes,
    totalTxBytes: tick.todayTxBytes,
    totalBytes: tick.todayTotalBytes,
    peakBps: 19_080_000,
    series: tick.sparklineToday,
    processes: tick.topProcessesToday,
  };
}
