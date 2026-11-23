import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  Eye,
  EyeOff,
  Globe,
  Lock,
  Minus,
  RefreshCw,
  Search,
  Settings,
  ShieldAlert,
  ShieldCheck,
  Square,
  X,
} from "lucide-react";
import xscapeIcon from "../assets/xscapenet-icon.png";
import { formatBytes, formatRate } from "../lib/format";
import {
  clearUsageHistory,
  closeCurrentWindow,
  fetchAnalytics,
  fetchBlockedApps,
  minimizeCurrentWindow,
  refreshPublicIp,
  relaunchAsAdmin,
  startWindowDrag,
  toggleAppBlock,
  toggleMaximizeCurrentWindow,
} from "../lib/tauri";
import type { AnalyticsResponse, AppSettings, LiveTick, TimeRange } from "../lib/types";
import { SettingsModal } from "./SettingsModal";
import { TrafficChart } from "./TrafficChart";

interface AdvancedViewProps {
  tick: LiveTick | null;
  settings: AppSettings;
  settingsError: string | null;
  onUpdateSettings: (next: AppSettings) => void;
}

const RANGES: Array<{ id: TimeRange; label: string; short: string }> = [
  { id: "hourly", label: "Hourly", short: "1H" },
  { id: "today", label: "Today", short: "Today" },
  { id: "two_days", label: "2 Days", short: "2D" },
  { id: "weekly", label: "Weekly", short: "7D" },
  { id: "monthly", label: "Monthly", short: "30D" },
];

const PROTECTED_PROCS = new Set([
  "system",
  "system idle",
  "ntoskrnl.exe",
  "svchost.exe",
  "lsass.exe",
  "csrss.exe",
  "wininit.exe",
  "services.exe",
  "explorer.exe",
  "dwm.exe",
  "winlogon.exe",
  "smss.exe",
  "xscapenet.exe",
]);

export const AdvancedView: React.FC<AdvancedViewProps> = ({
  tick,
  settings,
  settingsError,
  onUpdateSettings,
}) => {
  const [range, setRange] = useState<TimeRange>("today");
  const [interfaceId, setInterfaceId] = useState<string>("all");
  const [analytics, setAnalytics] = useState<AnalyticsResponse | null>(null);
  const [expandedProcs, setExpandedProcs] = useState<Record<string, boolean>>({});
  const [searchQuery, setSearchQuery] = useState("");
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [refreshingIp, setRefreshingIp] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [blockedApps, setBlockedApps] = useState<string[]>([]);
  const [firewallNotice, setFirewallNotice] = useState<string | null>(null);
  const [busyProc, setBusyProc] = useState<string | null>(null);

  const loadAnalytics = useCallback(async () => {
    try {
      const res = await fetchAnalytics(range, interfaceId);
      setAnalytics(res);
    } catch {
      /* keep previous snapshot */
    }
  }, [range, interfaceId]);

  useEffect(() => {
    void loadAnalytics();
  }, [loadAnalytics, tick?.timestamp]);

  useEffect(() => {
    void fetchBlockedApps()
      .then((list) => setBlockedApps(list.map((n) => n.toLowerCase())))
      .catch(() => {});
  }, []);

  const toggleExpand = (procName: string) => {
    setExpandedProcs((prev) => ({ ...prev, [procName]: !prev[procName] }));
  };

  const handleToggleBlock = async (
    e: React.MouseEvent,
    processName: string,
    processPath: string,
    currentlyBlocked: boolean
  ) => {
    e.stopPropagation();
    if (busyProc) return;
    setFirewallNotice(null);
    setBusyProc(processName);
    try {
      const updated = await toggleAppBlock(processName, processPath, !currentlyBlocked);
      setBlockedApps(updated.map((n) => n.toLowerCase()));
    } catch (err) {
      setFirewallNotice(typeof err === "string" ? err : String(err));
      window.setTimeout(() => setFirewallNotice(null), 5000);
    } finally {
      setBusyProc(null);
    }
  };

  const handleCopy = async (val: string, key: string) => {
    if (!val || val === "—" || settings.hideIp) return;
    try {
      await navigator.clipboard.writeText(val);
      setCopiedField(key);
      window.setTimeout(() => setCopiedField(null), 1400);
    } catch {
      /* ignore */
    }
  };

  const handleRefreshIp = async () => {
    setRefreshingIp(true);
    try {
      await refreshPublicIp();
    } finally {
      window.setTimeout(() => setRefreshingIp(false), 500);
    }
  };

  const handleClear = async () => {
    await clearUsageHistory();
    await loadAnalytics();
  };

  const filteredProcesses = useMemo(() => {
    const list = analytics?.processes ?? [];
    const q = searchQuery.trim().toLowerCase();
    if (!q) return list;
    return list.filter(
      (p) =>
        p.processName.toLowerCase().includes(q) ||
        p.endpoints.some((e) => e.remoteHost.toLowerCase().includes(q))
    );
  }, [analytics, searchQuery]);

  const id = tick?.identity;
  const interfaces = tick?.interfaces ?? [];
  const collectorMode = analytics?.collectorMode ?? tick?.collectorMode ?? "standard";
  const masked = "•••.•••.•••.•••";

  return (
    <div className="xscape-stage h-full w-full flex flex-col overflow-hidden">
      {/* Zero-Overlap Responsive Header */}
      <header
        onMouseDown={(e) => {
          if ((e.target as HTMLElement).closest("button, select, input")) return;
          void startWindowDrag();
        }}
        className="h-14 px-4 flex items-center justify-between gap-2 border-b border-white/[0.07] shrink-0 overflow-hidden"
      >
        {/* Left brand */}
        <div className="flex items-center gap-2.5 shrink-0">
          <img
            src={xscapeIcon}
            alt="XscapeNet"
            className="w-6 h-6 rounded-[6px] shrink-0"
            draggable={false}
          />
          <span className="text-[14.5px] font-semibold tracking-tight text-white whitespace-nowrap">
            XscapeNet
          </span>
        </div>

        {/* Center Segmented Pill */}
        <nav
          aria-label="Time range"
          className="lg-pill rounded-full p-1 flex items-center gap-0.5 shrink-0"
        >
          {RANGES.map((r) => {
            const active = range === r.id;
            return (
              <button
                key={r.id}
                type="button"
                onClick={() => setRange(r.id)}
                className={`h-7 px-3 rounded-full text-[11.5px] transition-all whitespace-nowrap shrink-0 ${
                  active
                    ? "lg-pill-active font-semibold text-white"
                    : "font-medium text-zinc-400 hover:text-white border border-transparent"
                }`}
              >
                <span className="hidden lg:inline">{r.label}</span>
                <span className="lg:hidden">{r.short}</span>
              </button>
            );
          })}
        </nav>

        {/* Right controls — cleanly collapses at narrow widths so NOTHING ever overlaps */}
        <div className="flex items-center gap-2 shrink-0">
          <select
            aria-label="Network interface filter"
            value={interfaceId}
            onChange={(e) => setInterfaceId(e.target.value)}
            className="hidden xl:block h-8 rounded-xl px-2.5 text-[11px] font-medium text-zinc-200 bg-white/[0.04] border border-white/[0.09] focus:outline-none cursor-pointer max-w-[130px] truncate"
          >
            <option value="all" className="bg-black">
              All Interfaces
            </option>
            {interfaces.map((iface) => (
              <option key={iface.id} value={iface.id} className="bg-black">
                {iface.name}
              </option>
            ))}
          </select>

          <div className="hidden lg:flex lg-pill rounded-full h-8 px-3 items-center gap-1.5 text-[11px] font-mono tabular-nums text-zinc-100 whitespace-nowrap shrink-0">
            <span
              className="h-1.5 w-1.5 rounded-full shrink-0"
              style={{ backgroundColor: "var(--x-rx)" }}
            />
            <span>{settings.hideIp ? masked : id?.publicIpv4 ?? "—"}</span>
            {!settings.hideIp && id?.countryCode && id.countryCode !== "--" && (
              <span className="hidden xl:inline text-zinc-400">· {id.countryCode}</span>
            )}
            <button
              type="button"
              onClick={() => onUpdateSettings({ ...settings, hideIp: !settings.hideIp })}
              aria-label={settings.hideIp ? "Show IP" : "Hide IP"}
              title={settings.hideIp ? "Show IP" : "Hide IP"}
              className="ml-0.5 p-1 rounded-full text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors"
            >
              {settings.hideIp ? (
                <EyeOff className="w-3 h-3" style={{ color: "var(--x-tx)" }} strokeWidth={1.7} />
              ) : (
                <Eye className="w-3 h-3" strokeWidth={1.6} />
              )}
            </button>
          </div>

          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            aria-label="Open settings"
            title="Preferences"
            className="h-8 w-8 rounded-xl flex items-center justify-center text-zinc-400 hover:text-white bg-white/[0.04] hover:bg-white/[0.09] border border-white/[0.09] transition-colors shrink-0"
          >
            <Settings className="w-3.5 h-3.5" strokeWidth={1.6} />
          </button>

          <div className="flex items-center gap-0.5 pl-1 border-l border-white/[0.08] shrink-0">
            <button
              type="button"
              onClick={() => void minimizeCurrentWindow()}
              aria-label="Minimize"
              className="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08]"
            >
              <Minus className="w-3.5 h-3.5" strokeWidth={1.6} />
            </button>
            <button
              type="button"
              onClick={() => void toggleMaximizeCurrentWindow()}
              aria-label="Maximize"
              className="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08]"
            >
              <Square className="w-3 h-3" strokeWidth={1.6} />
            </button>
            <button
              type="button"
              onClick={() => void closeCurrentWindow()}
              aria-label="Hide to tray"
              className="p-1.5 rounded-lg text-zinc-400 hover:text-rose-300 hover:bg-rose-500/15"
            >
              <X className="w-3.5 h-3.5" strokeWidth={1.6} />
            </button>
          </div>
        </div>
      </header>

      {/* Main Responsive Workspace */}
      <main className="flex-1 min-h-0 p-3.5 flex flex-col gap-3.5 overflow-hidden">
        {/* Upper Traffic Chart Panel */}
        <section className="lg-panel rounded-[20px] px-4 py-3.5 shrink-0">
          <div className="flex flex-wrap items-center justify-between gap-3 mb-2">
            <div className="flex items-center gap-4 font-mono tabular-nums min-w-0">
              <div className="shrink-0">
                <div className="text-[10px] font-sans uppercase tracking-wider text-zinc-500">
                  Range Total
                </div>
                <div className="text-[19px] font-semibold text-white leading-tight whitespace-nowrap">
                  {formatBytes(analytics?.totalBytes ?? 0)}
                </div>
              </div>
              <div className="h-6 w-px bg-white/[0.08] shrink-0" />
              <div
                className="flex items-center gap-1.5 text-[12.5px] whitespace-nowrap"
                style={{ color: "var(--x-rx)" }}
              >
                <ArrowDown className="w-3.5 h-3.5 shrink-0" strokeWidth={1.75} />
                <span>{formatBytes(analytics?.totalRxBytes ?? 0)}</span>
              </div>
              <div
                className="flex items-center gap-1.5 text-[12.5px] whitespace-nowrap"
                style={{ color: "var(--x-tx)" }}
              >
                <ArrowUp className="w-3.5 h-3.5 shrink-0" strokeWidth={1.75} />
                <span>{formatBytes(analytics?.totalTxBytes ?? 0)}</span>
              </div>
              <div className="hidden md:flex items-center gap-1 text-[11px] text-zinc-400 pl-2 border-l border-white/[0.08] whitespace-nowrap">
                <span>Peak</span>
                <span className="text-zinc-100">{formatRate(analytics?.peakBps ?? 0)}</span>
              </div>
            </div>

            <div className="lg-pill rounded-full px-3 py-1 flex items-center gap-2 text-[11px] font-mono tabular-nums whitespace-nowrap shrink-0">
              <span style={{ color: "var(--x-rx)" }}>↓ {formatRate(tick?.rxRateBps ?? 0)}</span>
              <span className="text-zinc-600">·</span>
              <span style={{ color: "var(--x-tx)" }}>↑ {formatRate(tick?.txRateBps ?? 0)}</span>
            </div>
          </div>

          <TrafficChart points={analytics?.series ?? []} />
        </section>

        {/* Lower Split */}
        <div className="flex-1 min-h-0 grid grid-cols-1 lg:grid-cols-12 gap-3.5 overflow-hidden">
          {/* Left: Per-Process Breakdown + Per-App Firewall Block Switch */}
          <section className="lg-panel rounded-[20px] lg:col-span-8 min-h-0 h-full flex flex-col overflow-hidden">
            <div className="px-4 py-2.5 border-b border-white/[0.07] flex items-center justify-between gap-2 shrink-0">
              <div className="flex items-center gap-2.5 min-w-0">
                <h2 className="text-[13px] font-semibold text-white whitespace-nowrap truncate">
                  Per-Process Breakdown
                </h2>
                {collectorMode === "kernel_etw" ? (
                  <span
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium whitespace-nowrap shrink-0 border"
                    style={{
                      backgroundColor: "var(--x-accent-soft)",
                      borderColor: "var(--x-glass-rim)",
                      color: "var(--x-rx)",
                    }}
                  >
                    <ShieldCheck className="w-3 h-3 shrink-0" strokeWidth={1.6} />
                    Kernel ETW
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => void relaunchAsAdmin()}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-500/12 border border-amber-500/25 text-[10px] font-medium text-amber-300 hover:bg-amber-500/20 transition-colors whitespace-nowrap shrink-0"
                  >
                    <ShieldAlert className="w-3 h-3 shrink-0" strokeWidth={1.6} />
                    Enable Admin ETW
                  </button>
                )}
              </div>

              <div className="relative shrink-0">
                <Search
                  className="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2"
                  strokeWidth={1.5}
                />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Filter app or domain..."
                  className="h-7 rounded-xl pl-8 pr-3 text-[11px] text-zinc-100 placeholder:text-zinc-500 bg-white/[0.04] border border-white/[0.08] focus:outline-none w-40 sm:w-48"
                />
              </div>
            </div>

            {firewallNotice && (
              <div className="px-4 py-1.5 bg-rose-500/12 border-b border-rose-500/25 text-[11px] text-rose-200 flex items-center justify-between shrink-0">
                <span>{firewallNotice}</span>
                <button
                  type="button"
                  onClick={() => setFirewallNotice(null)}
                  className="text-rose-300 hover:text-white ml-2"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            )}

            {/* Precision Fixed-Column Table Header including Net Switch */}
            <div className="grid grid-cols-[minmax(140px,1fr)_82px_82px_84px_70px_54px] gap-2.5 px-4 py-2 text-[11px] font-medium text-zinc-500 border-b border-white/[0.05] shrink-0">
              <div>Process / Domain</div>
              <div className="text-right whitespace-nowrap">Download</div>
              <div className="text-right whitespace-nowrap">Upload</div>
              <div className="text-right whitespace-nowrap">Total</div>
              <div className="text-right whitespace-nowrap">Share</div>
              <div className="text-right whitespace-nowrap">Access</div>
            </div>

            <div className="flex-1 overflow-y-auto clean-scroll divide-y divide-white/[0.04]">
              {filteredProcesses.length === 0 ? (
                <div className="h-full flex items-center justify-center text-[12px] text-zinc-500">
                  No process traffic recorded in this window yet.
                </div>
              ) : (
                filteredProcesses.map((proc) => {
                  const isOpen = !!expandedProcs[proc.processName];
                  const hasEndpoints = proc.endpoints.length > 0;
                  const isProtected = PROTECTED_PROCS.has(proc.processName.toLowerCase());
                  const isBlocked = blockedApps.includes(proc.processName.toLowerCase());
                  const isBusy = busyProc === proc.processName;
                  const canToggle = !isProtected && proc.processPath.trim().length > 0;

                  return (
                    <div key={proc.processName}>
                      <div
                        role="button"
                        tabIndex={0}
                        onClick={() => hasEndpoints && toggleExpand(proc.processName)}
                        onKeyDown={(e) => {
                          if ((e.key === "Enter" || e.key === " ") && hasEndpoints) {
                            toggleExpand(proc.processName);
                          }
                        }}
                        className="w-full grid grid-cols-[minmax(140px,1fr)_82px_82px_84px_70px_54px] gap-2.5 items-center px-4 py-2.5 text-[12px] hover:bg-white/[0.04] transition-colors cursor-pointer"
                      >
                        <div className="flex items-center gap-2 min-w-0">
                          {hasEndpoints ? (
                            isOpen ? (
                              <ChevronDown
                                className="w-3.5 h-3.5 shrink-0"
                                style={{ color: "var(--x-rx)" }}
                              />
                            ) : (
                              <ChevronRight className="w-3.5 h-3.5 text-zinc-500 shrink-0" />
                            )
                          ) : (
                            <span className="w-3.5 shrink-0" />
                          )}
                          <span
                            className={`font-medium truncate ${
                              isBlocked ? "text-rose-300 line-through opacity-80" : "text-zinc-100"
                            }`}
                          >
                            {proc.processName}
                          </span>
                          {proc.activeConnections > 0 && (
                            <span className="text-[10px] font-mono text-zinc-500 shrink-0">
                              {proc.activeConnections}
                            </span>
                          )}
                        </div>
                        <div
                          className="text-right font-mono tabular-nums text-[11.5px] whitespace-nowrap"
                          style={{ color: "var(--x-rx)" }}
                        >
                          {formatBytes(proc.rxBytes)}
                        </div>
                        <div
                          className="text-right font-mono tabular-nums text-[11.5px] whitespace-nowrap"
                          style={{ color: "var(--x-tx)" }}
                        >
                          {formatBytes(proc.txBytes)}
                        </div>
                        <div className="text-right font-mono tabular-nums text-white text-[11.5px] font-medium whitespace-nowrap">
                          {proc.totalBytes > 0
                            ? formatBytes(proc.totalBytes)
                            : `${proc.activeConnections} sock`}
                        </div>
                        <div className="flex items-center justify-end">
                          <div className="w-full h-1.5 rounded-full bg-white/[0.08] overflow-hidden">
                            <div
                              className="h-full rounded-full"
                              style={{
                                width: `${Math.max(5, Math.min(100, proc.sharePercent))}%`,
                                backgroundColor: isBlocked ? "#f43f5e" : "var(--x-bar)",
                              }}
                            />
                          </div>
                        </div>

                        {/* Safe Per-App Firewall Allow/Block Slide Switch */}
                        <div className="flex items-center justify-end">
                          {isProtected ? (
                            <span
                              title="Protected Windows system process"
                              className="p-1 text-zinc-600"
                            >
                              <Lock className="w-3 h-3" strokeWidth={1.5} />
                            </span>
                          ) : (
                            <button
                              type="button"
                              role="switch"
                              disabled={!canToggle || isBusy}
                              aria-checked={!isBlocked}
                              title={
                                !canToggle
                                  ? "Executable path unavailable (run as Admin)"
                                  : isBlocked
                                    ? `Blocked via Windows Firewall — click to allow ${proc.processName}`
                                    : `Allowed — click to block ${proc.processName} internet access`
                              }
                              onClick={(e) =>
                                void handleToggleBlock(
                                  e,
                                  proc.processName,
                                  proc.processPath,
                                  isBlocked
                                )
                              }
                              className={`relative h-[18px] w-[32px] rounded-full transition-colors duration-200 shrink-0 ${
                                !canToggle ? "opacity-35 cursor-not-allowed" : "cursor-pointer"
                              } ${isBlocked ? "bg-rose-500/35 border border-rose-500/50" : ""}`}
                              style={
                                !isBlocked
                                  ? { backgroundColor: "var(--x-accent)" }
                                  : undefined
                              }
                            >
                              <span
                                className={`block h-3 w-3 rounded-full shadow transition-transform duration-200 ${
                                  isBlocked ? "translate-x-[3px] bg-rose-200" : "translate-x-[17px]"
                                }`}
                                style={
                                  !isBlocked
                                    ? { backgroundColor: "var(--x-accent-fg)" }
                                    : undefined
                                }
                              />
                            </button>
                          )}
                        </div>
                      </div>

                      {isOpen && hasEndpoints && (
                        <div className="bg-black/50 border-t border-white/[0.04] divide-y divide-white/[0.03]">
                          {proc.endpoints.map((ep) => (
                            <div
                              key={`${proc.processName}-${ep.remoteHost}-${ep.protocol}`}
                              className="grid grid-cols-[minmax(140px,1fr)_82px_82px_84px_70px_54px] gap-2.5 items-center px-4 py-1.5 text-[11px]"
                            >
                              <div className="pl-6 flex items-center gap-2 min-w-0">
                                <Globe
                                  className="w-3 h-3 text-zinc-500 shrink-0"
                                  strokeWidth={1.5}
                                />
                                <span className="text-zinc-300 font-mono truncate">
                                  {settings.hideIp ? "••••••••.•••" : ep.remoteHost}
                                </span>
                                <span className="px-1.5 py-0.5 rounded bg-white/[0.07] text-[9.5px] font-mono text-zinc-300 shrink-0">
                                  {ep.protocol}
                                </span>
                              </div>
                              <div className="text-right font-mono tabular-nums text-zinc-400 whitespace-nowrap">
                                {formatBytes(ep.rxBytes)}
                              </div>
                              <div className="text-right font-mono tabular-nums text-zinc-400 whitespace-nowrap">
                                {formatBytes(ep.txBytes)}
                              </div>
                              <div className="text-right font-mono tabular-nums text-zinc-200 whitespace-nowrap">
                                {formatBytes(ep.totalBytes)}
                              </div>
                              <div />
                              <div />
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })
              )}
            </div>
          </section>

          {/* Right: Network Identity Sidebar */}
          <aside className="lg-panel rounded-[20px] hidden lg:flex lg:col-span-4 min-h-0 h-full px-4 py-3.5 flex-col justify-between overflow-hidden">
            <div>
              <div className="flex items-center justify-between mb-2.5">
                <h2 className="text-[13px] font-semibold text-white">Network Identity</h2>
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() =>
                      onUpdateSettings({ ...settings, hideIp: !settings.hideIp })
                    }
                    aria-label={settings.hideIp ? "Show IP" : "Hide IP"}
                    title={settings.hideIp ? "Show IP" : "Hide IP"}
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.07] transition-colors"
                  >
                    {settings.hideIp ? (
                      <EyeOff
                        className="w-3.5 h-3.5"
                        style={{ color: "var(--x-tx)" }}
                        strokeWidth={1.6}
                      />
                    ) : (
                      <Eye className="w-3.5 h-3.5" strokeWidth={1.6} />
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleRefreshIp()}
                    aria-label="Refresh public IP"
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.07] transition-colors"
                  >
                    <RefreshCw
                      className={`w-3.5 h-3.5 ${refreshingIp ? "animate-spin" : ""}`}
                      style={refreshingIp ? { color: "var(--x-rx)" } : undefined}
                      strokeWidth={1.5}
                    />
                  </button>
                </div>
              </div>

              <dl className="divide-y divide-white/[0.055] text-[12px]">
                <div className="py-2.5 flex items-center justify-between gap-2">
                  <dt className="text-zinc-500 whitespace-nowrap">Public IPv4</dt>
                  <dd className="flex items-center gap-1.5 font-mono tabular-nums text-white truncate">
                    <span className={settings.hideIp ? "tracking-wider text-zinc-400" : ""}>
                      {settings.hideIp ? masked : id?.publicIpv4 ?? "Detecting..."}
                    </span>
                    {!settings.hideIp && (
                      <button
                        type="button"
                        onClick={() => void handleCopy(id?.publicIpv4 ?? "", "pub")}
                        aria-label="Copy Public IP"
                        className="p-0.5 text-zinc-500 hover:text-white shrink-0"
                      >
                        {copiedField === "pub" ? (
                          <Check
                            className="w-3 h-3"
                            style={{ color: "var(--x-rx)" }}
                            strokeWidth={2}
                          />
                        ) : (
                          <Copy className="w-3 h-3" strokeWidth={1.5} />
                        )}
                      </button>
                    )}
                  </dd>
                </div>

                <div className="py-2.5 flex items-center justify-between gap-2">
                  <dt className="text-zinc-500 whitespace-nowrap">ASN / ISP</dt>
                  <dd className="text-zinc-200 text-right truncate">
                    {settings.hideIp ? "Hidden" : id?.ispAsn ?? "Resolving..."}
                  </dd>
                </div>

                <div className="py-2.5 flex items-center justify-between gap-2">
                  <dt className="text-zinc-500 whitespace-nowrap">Local IP</dt>
                  <dd className="flex items-center gap-1.5 font-mono tabular-nums text-zinc-200">
                    <span className={settings.hideIp ? "tracking-wider text-zinc-400" : ""}>
                      {settings.hideIp ? masked : id?.localIp ?? "—"}
                    </span>
                    {!settings.hideIp && (
                      <button
                        type="button"
                        onClick={() => void handleCopy(id?.localIp ?? "", "lan")}
                        aria-label="Copy Local IP"
                        className="p-0.5 text-zinc-500 hover:text-white shrink-0"
                      >
                        {copiedField === "lan" ? (
                          <Check
                            className="w-3 h-3"
                            style={{ color: "var(--x-rx)" }}
                            strokeWidth={2}
                          />
                        ) : (
                          <Copy className="w-3 h-3" strokeWidth={1.5} />
                        )}
                      </button>
                    )}
                  </dd>
                </div>

                <div className="py-2.5 flex items-center justify-between gap-2">
                  <dt className="text-zinc-500 whitespace-nowrap">Gateway</dt>
                  <dd
                    className={`font-mono tabular-nums ${
                      settings.hideIp ? "tracking-wider text-zinc-400" : "text-zinc-300"
                    }`}
                  >
                    {settings.hideIp ? masked : id?.gateway ?? "—"}
                  </dd>
                </div>

                <div className="py-2.5 flex items-center justify-between gap-2">
                  <dt className="text-zinc-500 whitespace-nowrap">DNS</dt>
                  <dd className="font-mono tabular-nums text-zinc-300 text-right truncate">
                    {settings.hideIp
                      ? "Hidden"
                      : id?.dnsServers?.length
                        ? id.dnsServers.slice(0, 2).join(", ")
                        : "—"}
                  </dd>
                </div>

                <div className="py-2.5 flex items-center justify-between gap-2">
                  <dt className="text-zinc-500 whitespace-nowrap">Active Sockets</dt>
                  <dd className="font-mono tabular-nums text-zinc-200 whitespace-nowrap">
                    {id?.activeTcpSockets ?? 0} TCP / {id?.activeUdpSockets ?? 0} UDP
                  </dd>
                </div>
              </dl>
            </div>

            {interfaces.length > 0 && (
              <div className="pt-2.5 border-t border-white/[0.06]">
                <div className="text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
                  Active Interfaces Today
                </div>
                <div className="space-y-1.5">
                  {interfaces.slice(0, 3).map((iface) => (
                    <div
                      key={iface.id}
                      className="flex items-center justify-between text-[11.5px] gap-2"
                    >
                      <span className="flex items-center gap-1.5 min-w-0">
                        <span className="text-zinc-200 truncate">{iface.name}</span>
                        <span className="px-1.5 py-0.5 rounded bg-white/[0.06] text-[9.5px] text-zinc-400 shrink-0">
                          {iface.kind}
                        </span>
                      </span>
                      <span className="font-mono tabular-nums text-zinc-300 whitespace-nowrap">
                        {formatBytes(iface.rxTodayBytes + iface.txTodayBytes)}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </aside>
        </div>
      </main>

      <SettingsModal
        open={settingsOpen}
        settings={settings}
        statusError={settingsError}
        onClose={() => setSettingsOpen(false)}
        onUpdate={onUpdateSettings}
        onClearHistory={handleClear}
      />
    </div>
  );
};
