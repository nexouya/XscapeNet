import React, { useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpRight,
  Check,
  Copy,
  Eye,
  EyeOff,
  RefreshCw,
  ShieldCheck,
  X,
} from "lucide-react";
import xscapeIcon from "../assets/xscapenet-icon.png";
import { cleanProcessLabel, formatBytes, formatRate } from "../lib/format";
import { hidePopoverWindow, openAdvancedWindow, refreshPublicIp } from "../lib/tauri";
import type { AppSettings, LiveTick } from "../lib/types";
import { TrafficChart } from "./TrafficChart";

interface PopoverViewProps {
  tick: LiveTick | null;
  settings: AppSettings;
  onUpdateSettings: (next: AppSettings) => void;
}

export const PopoverView: React.FC<PopoverViewProps> = ({
  tick,
  settings,
  onUpdateSettings,
}) => {
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const handleCopy = async (value: string, key: string) => {
    if (!value || value === "—" || value.startsWith("Detect") || settings.hideIp) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopiedKey(key);
      window.setTimeout(() => setCopiedKey(null), 1400);
    } catch {
      /* ignore */
    }
  };

  const handleRefreshIp = async () => {
    setRefreshing(true);
    try {
      await refreshPublicIp();
    } finally {
      window.setTimeout(() => setRefreshing(false), 500);
    }
  };

  const id = tick?.identity;
  const topApps = (tick?.topProcessesToday ?? []).slice(0, 3);
  const maskedIp = "•••.•••.•••.•••";

  return (
    <div className="xscape-stage h-full w-full flex flex-col justify-between p-3.5 overflow-hidden">
      {/* Top bar */}
      <div className="flex items-center justify-between gap-2 shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <img
            src={xscapeIcon}
            alt="XscapeNet"
            className="w-5 h-5 rounded-[5px] shrink-0"
            draggable={false}
          />
          <span className="text-[14px] font-semibold tracking-tight text-white whitespace-nowrap">
            XscapeNet
          </span>
          <span className="inline-flex items-center gap-1.5 text-[11px] text-zinc-400 truncate">
            <span
              className="h-1.5 w-1.5 rounded-full shrink-0"
              style={{ backgroundColor: "var(--x-rx)" }}
            />
            <span className="truncate">{id?.activeInterface ?? "Connecting"}</span>
          </span>
        </div>

        <div className="flex items-center gap-0.5 shrink-0">
          <button
            type="button"
            onClick={() => onUpdateSettings({ ...settings, hideIp: !settings.hideIp })}
            aria-label={settings.hideIp ? "Show IP" : "Hide IP"}
            title={settings.hideIp ? "Show IP" : "Hide IP"}
            className="p-1.5 rounded-full text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors"
          >
            {settings.hideIp ? (
              <EyeOff className="w-3.5 h-3.5" style={{ color: "var(--x-tx)" }} strokeWidth={1.6} />
            ) : (
              <Eye className="w-3.5 h-3.5" strokeWidth={1.6} />
            )}
          </button>
          <button
            type="button"
            onClick={() => void hidePopoverWindow()}
            aria-label="Hide popover"
            className="p-1.5 rounded-full text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors"
          >
            <X className="w-3.5 h-3.5" strokeWidth={1.6} />
          </button>
        </div>
      </div>

      {/* Liquid Glass IP Identity Pill */}
      <div className="lg-pill rounded-2xl mt-2.5 px-3 py-2 flex items-center justify-between gap-2 text-[11px] font-mono tabular-nums shrink-0">
        <div className="flex items-center gap-1.5 min-w-0">
          <span className="text-zinc-500 font-sans shrink-0">IP</span>
          <span className="text-zinc-100 truncate">
            {settings.hideIp ? maskedIp : id?.publicIpv4 ?? "Detecting..."}
          </span>
          {!settings.hideIp && id?.countryCode && id.countryCode !== "--" && (
            <span className="px-1.5 py-0.5 rounded bg-white/[0.08] text-[10px] text-zinc-200 font-sans font-medium shrink-0">
              {id.countryCode}
            </span>
          )}
          {!settings.hideIp && (
            <button
              type="button"
              onClick={() => void handleCopy(id?.publicIpv4 ?? "", "pub")}
              aria-label="Copy public IP"
              className="p-1 rounded text-zinc-400 hover:text-white transition-colors shrink-0"
            >
              {copiedKey === "pub" ? (
                <Check className="w-3 h-3" style={{ color: "var(--x-rx)" }} strokeWidth={2} />
              ) : (
                <Copy className="w-3 h-3" strokeWidth={1.5} />
              )}
            </button>
          )}
        </div>

        <div className="flex items-center gap-1.5 pl-2 border-l border-white/[0.08] shrink-0">
          <span className="text-zinc-500 font-sans">LAN</span>
          <span className="text-zinc-300">
            {settings.hideIp ? "•••.•••.•••" : id?.localIp ?? "—"}
          </span>
          <button
            type="button"
            onClick={() => void handleRefreshIp()}
            aria-label="Refresh IP"
            className="p-0.5 rounded text-zinc-400 hover:text-white transition-colors"
          >
            <RefreshCw
              className={`w-3 h-3 ${refreshing ? "animate-spin" : ""}`}
              style={refreshing ? { color: "var(--x-rx)" } : undefined}
              strokeWidth={1.5}
            />
          </button>
        </div>
      </div>

      {/* Main Liquid Glass Hero Today Slab */}
      <div className="lg-panel rounded-[20px] mt-2.5 flex-1 min-h-0 p-3.5 flex flex-col justify-between">
        <div>
          <div className="flex items-baseline justify-between">
            <div className="font-mono tabular-nums text-[30px] font-semibold leading-none tracking-tight text-white">
              {formatBytes(tick?.todayTotalBytes ?? 0)}
            </div>
            <span className="text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
              Today
            </span>
          </div>

          <div className="mt-2.5 flex items-center gap-4 font-mono tabular-nums text-[12.5px]">
            <span
              className="inline-flex items-center gap-1 font-medium whitespace-nowrap"
              style={{ color: "var(--x-rx)" }}
            >
              <ArrowDown className="w-3.5 h-3.5" strokeWidth={1.75} />
              {formatBytes(tick?.todayRxBytes ?? 0)}
            </span>
            <span
              className="inline-flex items-center gap-1 font-medium whitespace-nowrap"
              style={{ color: "var(--x-tx)" }}
            >
              <ArrowUp className="w-3.5 h-3.5" strokeWidth={1.75} />
              {formatBytes(tick?.todayTxBytes ?? 0)}
            </span>
          </div>

          <div className="lg-pill rounded-full mt-2.5 inline-flex items-center gap-2 px-2.5 py-1 text-[11px] font-mono tabular-nums text-zinc-200 whitespace-nowrap">
            <span style={{ color: "var(--x-rx)" }}>↓ {formatRate(tick?.rxRateBps ?? 0)}</span>
            <span className="text-zinc-600">·</span>
            <span style={{ color: "var(--x-tx)" }}>↑ {formatRate(tick?.txRateBps ?? 0)}</span>
          </div>
        </div>

        <div className="my-1">
          <TrafficChart points={tick?.sparklineToday ?? []} compact />
        </div>

        <div className="pt-2 border-t border-white/[0.07]">
          <div className="flex items-center justify-between mb-2">
            <span className="text-[11px] font-medium text-zinc-400">Top Apps Today</span>
            {tick?.collectorMode === "kernel_etw" && (
              <span
                className="inline-flex items-center gap-1 text-[10px]"
                style={{ color: "var(--x-rx)" }}
              >
                <ShieldCheck className="w-3 h-3" strokeWidth={1.5} />
                ETW
              </span>
            )}
          </div>

          {topApps.length === 0 ? (
            <div className="text-[11px] text-zinc-500 py-1.5">
              Collecting per-process telemetry...
            </div>
          ) : (
            <div className="space-y-2">
              {topApps.map((app, idx) => (
                <div key={app.processName} className="space-y-1">
                  <div className="flex items-center justify-between text-[12px]">
                    <span className="text-zinc-100 font-medium truncate max-w-[185px]">
                      {cleanProcessLabel(app.processName)}
                    </span>
                    <span className="font-mono tabular-nums text-zinc-300 text-[11px] whitespace-nowrap">
                      {app.totalBytes > 0
                        ? formatBytes(app.totalBytes)
                        : `${app.activeConnections} conn`}
                    </span>
                  </div>
                  <div className="h-[3px] w-full rounded-full bg-white/[0.08] overflow-hidden">
                    <div
                      className="h-full rounded-full"
                      style={{
                        width: `${Math.max(4, Math.min(100, app.sharePercent))}%`,
                        backgroundColor: idx === 0 ? "var(--x-rx)" : "var(--x-tx)",
                      }}
                    />
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Bottom CTA */}
      <button
        type="button"
        onClick={() => void openAdvancedWindow()}
        className="lg-cta mt-2.5 w-full rounded-full py-2.5 px-4 text-[12px] font-medium text-white flex items-center justify-center gap-1.5 whitespace-nowrap shrink-0 group"
      >
        <span>Advanced Analytics</span>
        <ArrowUpRight
          className="w-3.5 h-3.5 transition-transform duration-150 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
          style={{ color: "var(--x-rx)" }}
          strokeWidth={1.75}
        />
      </button>
    </div>
  );
};
