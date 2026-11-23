export type TimeRange = "hourly" | "today" | "two_days" | "weekly" | "monthly";

export type CollectorMode = "kernel_etw" | "standard";

export type AppThemeId = "oled_black" | "obsidian" | "violet" | "emerald";
export type GlassIntensity = "subtle" | "liquid" | "ultra";

export interface AppSettings {
  theme: AppThemeId;
  hideIp: boolean;
  glassIntensity: GlassIntensity;
  runOnStartup: boolean;
}

export interface InterfaceInfo {
  id: string;
  name: string;
  description: string;
  kind: string;
  isUp: boolean;
  linkSpeedMbps: number;
  rxRateBps: number;
  txRateBps: number;
  rxTodayBytes: number;
  txTodayBytes: number;
}

export interface NetworkIdentity {
  publicIpv4: string;
  countryCode: string;
  countryName: string;
  ispAsn: string;
  latencyMs: number | null;
  localIp: string;
  gateway: string;
  dnsServers: string[];
  activeInterface: string;
  activeInterfaceKind: string;
  activeTcpSockets: number;
  activeUdpSockets: number;
  updatedAt: number;
}

export interface EndpointUsageRow {
  remoteHost: string;
  protocol: string;
  rxBytes: number;
  txBytes: number;
  totalBytes: number;
}

export interface ProcessUsageRow {
  processName: string;
  processPath: string;
  rxBytes: number;
  txBytes: number;
  totalBytes: number;
  sharePercent: number;
  activeConnections: number;
  endpoints: EndpointUsageRow[];
}

export interface TimeSeriesPoint {
  timestamp: number;
  label: string;
  rxBytes: number;
  txBytes: number;
  peakBps: number;
}

export interface LiveTick {
  timestamp: number;
  collectorMode: CollectorMode;
  rxRateBps: number;
  txRateBps: number;
  todayRxBytes: number;
  todayTxBytes: number;
  todayTotalBytes: number;
  interfaces: InterfaceInfo[];
  identity: NetworkIdentity;
  topProcessesToday: ProcessUsageRow[];
  sparklineToday: TimeSeriesPoint[];
}

export interface AnalyticsResponse {
  range: TimeRange;
  interfaceId: string;
  collectorMode: CollectorMode;
  totalRxBytes: number;
  totalTxBytes: number;
  totalBytes: number;
  peakBps: number;
  series: TimeSeriesPoint[];
  processes: ProcessUsageRow[];
}
