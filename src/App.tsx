import React, { useCallback, useEffect, useState } from "react";
import { AdvancedView } from "./components/AdvancedView";
import { PopoverView } from "./components/PopoverView";
import {
  DEFAULT_SETTINGS,
  fetchLiveSnapshot,
  fetchSettings,
  saveSettings,
  subscribeLiveTicks,
  subscribeSettings,
} from "./lib/tauri";
import type { AppSettings, LiveTick } from "./lib/types";

export const App: React.FC = () => {
  const [tick, setTick] = useState<LiveTick | null>(null);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [settingsError, setSettingsError] = useState<string | null>(null);

  const view =
    typeof window !== "undefined"
      ? new URLSearchParams(window.location.search).get("view") ?? "advanced"
      : "advanced";

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", "dark");
    document.documentElement.classList.add("dark");
    document.documentElement.setAttribute("data-xscape-theme", settings.theme);
    document.documentElement.setAttribute("data-glass", settings.glassIntensity);
  }, [settings.theme, settings.glassIntensity]);

  useEffect(() => {
    let active = true;
    let unlistenTick: (() => void) | undefined;
    let unlistenSettings: (() => void) | undefined;

    const syncSettings = () => {
      void fetchSettings().then((s) => {
        if (active && s) setSettings(s);
      });
    };

    syncSettings();

    void fetchLiveSnapshot().then((snap) => {
      if (active && snap) setTick(snap);
    });

    void subscribeLiveTicks((next) => {
      if (active) setTick(next);
    }).then((fn) => {
      if (active) unlistenTick = fn;
      else fn();
    });

    void subscribeSettings((next) => {
      if (active) setSettings(next);
    }).then((fn) => {
      if (active) unlistenSettings = fn;
      else fn();
    });

    // Re-sync settings whenever popover or main window regains focus/visibility
    const onVisible = () => {
      if (document.visibilityState === "visible") syncSettings();
    };
    window.addEventListener("focus", syncSettings);
    document.addEventListener("visibilitychange", onVisible);

    return () => {
      active = false;
      window.removeEventListener("focus", syncSettings);
      document.removeEventListener("visibilitychange", onVisible);
      if (unlistenTick) unlistenTick();
      if (unlistenSettings) unlistenSettings();
    };
  }, []);

  const handleUpdateSettings = useCallback((next: AppSettings) => {
    setSettingsError(null);
    setSettings(next);
    void saveSettings(next)
      .then((saved) => setSettings(saved))
      .catch((err) => {
        setSettingsError(typeof err === "string" ? err : String(err));
        void fetchSettings().then((actual) => setSettings(actual));
      });
  }, []);

  return view === "popover" ? (
    <PopoverView
      tick={tick}
      settings={settings}
      onUpdateSettings={handleUpdateSettings}
    />
  ) : (
    <AdvancedView
      tick={tick}
      settings={settings}
      settingsError={settingsError}
      onUpdateSettings={handleUpdateSettings}
    />
  );
};

export default App;
