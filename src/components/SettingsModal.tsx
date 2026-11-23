import React, { useState } from "react";
import { Check, EyeOff, Power, Sparkles, Trash2, X } from "lucide-react";
import type { AppSettings, AppThemeId, GlassIntensity } from "../lib/types";

interface SettingsModalProps {
  open: boolean;
  settings: AppSettings;
  statusError: string | null;
  onClose: () => void;
  onUpdate: (next: AppSettings) => void;
  onClearHistory: () => Promise<void>;
}

const THEMES: Array<{
  id: AppThemeId;
  label: string;
  swatch: string;
  ring: string;
}> = [
  { id: "oled_black", label: "OLED Black", swatch: "#000000", ring: "#E2E8F0" },
  { id: "violet", label: "Midnight Violet", swatch: "#140729", ring: "#A855F7" },
  { id: "obsidian", label: "Obsidian Slate", swatch: "#0A0E17", ring: "#38BDF8" },
  { id: "emerald", label: "Emerald Noir", swatch: "#03150E", ring: "#10B981" },
];

const GLASS_MODES: Array<{ id: GlassIntensity; label: string }> = [
  { id: "subtle", label: "Subtle" },
  { id: "liquid", label: "Liquid" },
  { id: "ultra", label: "Ultra" },
];

export const SettingsModal: React.FC<SettingsModalProps> = ({
  open,
  settings,
  statusError,
  onClose,
  onUpdate,
  onClearHistory,
}) => {
  const [confirmClear, setConfirmClear] = useState(false);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-md p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="lg-panel w-full max-w-[420px] rounded-[22px] p-5 flex flex-col gap-5">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-[15px] font-semibold tracking-tight text-white">
              Preferences
            </h3>
            <p className="text-[11px] text-zinc-400 mt-0.5">
              Saved permanently in your local SQLite database
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close settings"
            className="p-1.5 rounded-full text-zinc-400 hover:text-white hover:bg-white/[0.08] transition-colors"
          >
            <X className="w-4 h-4" strokeWidth={1.6} />
          </button>
        </div>

        {/* Theme Selector */}
        <div className="space-y-2">
          <div className="text-[11px] font-medium uppercase tracking-wider text-zinc-400">
            Appearance Theme
          </div>
          <div className="grid grid-cols-2 gap-2">
            {THEMES.map((t) => {
              const active = settings.theme === t.id;
              return (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => onUpdate({ ...settings, theme: t.id })}
                  className={`flex items-center justify-between rounded-xl px-3 py-2.5 text-[12px] font-medium transition-all border ${
                    active
                      ? "bg-white/[0.11] border-white/25 text-white"
                      : "bg-white/[0.03] border-white/[0.07] text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.06]"
                  }`}
                >
                  <span className="flex items-center gap-2 whitespace-nowrap">
                    <span
                      className="h-3.5 w-3.5 rounded-full border border-white/25 shrink-0"
                      style={{
                        backgroundColor: t.swatch,
                        boxShadow: active ? `0 0 0 2px ${t.ring}` : undefined,
                      }}
                    />
                    {t.label}
                  </span>
                  {active && (
                    <Check
                      className="w-3.5 h-3.5 shrink-0"
                      style={{ color: "var(--x-rx)" }}
                    />
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* Liquid Glass Intensity */}
        <div className="space-y-2">
          <div className="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-zinc-400">
            <Sparkles className="w-3.5 h-3.5" strokeWidth={1.5} />
            <span>Liquid Glass Depth</span>
          </div>
          <div className="grid grid-cols-3 gap-1.5 rounded-xl bg-white/[0.04] p-1 border border-white/[0.07]">
            {GLASS_MODES.map((m) => {
              const active = settings.glassIntensity === m.id;
              return (
                <button
                  key={m.id}
                  type="button"
                  onClick={() => onUpdate({ ...settings, glassIntensity: m.id })}
                  className={`py-1.5 rounded-lg text-[12px] font-medium transition-all whitespace-nowrap ${
                    active
                      ? "lg-pill-active text-white"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  {m.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Toggles (using div rows so buttons NEVER double-fire!) */}
        <div className="space-y-3 pt-2 border-t border-white/[0.07]">
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2.5 min-w-0">
              <Power
                className="w-4 h-4 shrink-0"
                style={{ color: "var(--x-rx)" }}
                strokeWidth={1.6}
              />
              <div className="min-w-0">
                <div className="text-[12.5px] font-medium text-zinc-100">
                  Run on Windows Startup
                </div>
                <div className="text-[11px] text-zinc-500">
                  Uses Task Scheduler (Admin ETW) or Startup Registry
                </div>
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.runOnStartup}
              onClick={() =>
                onUpdate({ ...settings, runOnStartup: !settings.runOnStartup })
              }
              className={`relative h-6 w-11 rounded-full transition-colors duration-200 shrink-0 ${
                settings.runOnStartup
                  ? ""
                  : "bg-white/[0.12] border border-white/[0.12]"
              }`}
              style={
                settings.runOnStartup ? { backgroundColor: "var(--x-accent)" } : undefined
              }
            >
              <span
                className={`block h-4 w-4 rounded-full shadow transition-transform duration-200 ${
                  settings.runOnStartup ? "translate-x-6" : "translate-x-1"
                }`}
                style={{
                  backgroundColor: settings.runOnStartup
                    ? "var(--x-accent-fg)"
                    : "#ffffff",
                }}
              />
            </button>
          </div>

          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2.5 min-w-0">
              <EyeOff
                className="w-4 h-4 shrink-0"
                style={{ color: "var(--x-tx)" }}
                strokeWidth={1.6}
              />
              <div className="min-w-0">
                <div className="text-[12.5px] font-medium text-zinc-100">
                  Privacy Mode (Hide IP)
                </div>
                <div className="text-[11px] text-zinc-500">
                  Mask Public IP, Local IP, Gateway &amp; ISP
                </div>
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.hideIp}
              onClick={() => onUpdate({ ...settings, hideIp: !settings.hideIp })}
              className={`relative h-6 w-11 rounded-full transition-colors duration-200 shrink-0 ${
                settings.hideIp ? "" : "bg-white/[0.12] border border-white/[0.12]"
              }`}
              style={settings.hideIp ? { backgroundColor: "var(--x-accent)" } : undefined}
            >
              <span
                className={`block h-4 w-4 rounded-full shadow transition-transform duration-200 ${
                  settings.hideIp ? "translate-x-6" : "translate-x-1"
                }`}
                style={{
                  backgroundColor: settings.hideIp ? "var(--x-accent-fg)" : "#ffffff",
                }}
              />
            </button>
          </div>

          {statusError && (
            <div className="rounded-xl bg-rose-500/12 border border-rose-500/25 px-3 py-2 text-[11px] text-rose-200">
              {statusError}
            </div>
          )}
        </div>

        {/* Clear History */}
        <div className="pt-3 border-t border-white/[0.07] flex items-center justify-between gap-3">
          <div className="text-[11px] text-zinc-500">
            History persists across restarts automatically.
          </div>
          {!confirmClear ? (
            <button
              type="button"
              onClick={() => setConfirmClear(true)}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11.5px] font-medium text-rose-300 bg-rose-500/10 border border-rose-500/20 hover:bg-rose-500/20 transition-colors whitespace-nowrap"
            >
              <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
              Clear Data
            </button>
          ) : (
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => setConfirmClear(false)}
                className="px-2.5 py-1.5 rounded-lg text-[11px] text-zinc-400 hover:text-white"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={async () => {
                  await onClearHistory();
                  setConfirmClear(false);
                }}
                className="px-3 py-1.5 rounded-lg text-[11px] font-medium text-white bg-rose-600 hover:bg-rose-500 transition-colors whitespace-nowrap"
              >
                Confirm Reset
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
