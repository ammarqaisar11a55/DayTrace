import { listen } from "@tauri-apps/api/event";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { api, errorMessage } from "../services/api";
import type { Settings, TrackerStatus } from "../types";

interface Toast {
  id: number;
  message: string;
  tone: "info" | "error";
}

interface AppContextValue {
  settings: Settings;
  status: TrackerStatus | null;
  saveSettings: (changes: Partial<Settings>) => Promise<void>;
  replaceSettings: (settings: Settings) => void;
  notify: (message: string, tone?: Toast["tone"]) => void;
  /** Runs an action and reports its failure as a toast. */
  attempt: <T>(action: () => Promise<T>) => Promise<T | undefined>;
  toasts: Toast[];
}

const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const value = useContext(AppContext);
  if (!value) throw new Error("useApp must be used inside AppProvider");
  return value;
}

function applyTheme(theme: Settings["theme"]) {
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

export function AppProvider({ children, fallback }: { children: ReactNode; fallback: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<TrackerStatus | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);

  const notify = useCallback((message: string, tone: Toast["tone"] = "info") => {
    const id = Date.now() + Math.random();
    setToasts((current) => [...current, { id, message, tone }]);
    window.setTimeout(() => setToasts((current) => current.filter((toast) => toast.id !== id)), 6_000);
  }, []);

  const attempt = useCallback(
    async <T,>(action: () => Promise<T>) => {
      try {
        return await action();
      } catch (error) {
        notify(errorMessage(error), "error");
        return undefined;
      }
    },
    [notify],
  );

  useEffect(() => {
    void api.getSettings().then(setSettings);
    void api.trackerStatus().then(setStatus);
    const unlisten = listen<TrackerStatus>("tracker-status", (event) => setStatus(event.payload));
    return () => void unlisten.then((stop) => stop());
  }, []);

  useEffect(() => {
    const announce = (version: string) => notify(`DayTrace ${version} is available. Open Settings to update.`);
    void api.knownUpdate().then((update) => update && announce(update.version));
    const unlisten = listen<string>("update-available", (event) => announce(event.payload));
    return () => void unlisten.then((stop) => stop());
  }, [notify]);

  useEffect(() => {
    if (settings) applyTheme(settings.theme);
  }, [settings?.theme]); // eslint-disable-line react-hooks/exhaustive-deps

  const saveSettings = useCallback(
    async (changes: Partial<Settings>) => {
      if (!settings) return;
      const saved = await attempt(() => api.updateSettings({ ...settings, ...changes }));
      if (saved) setSettings(saved);
    },
    [attempt, settings],
  );

  const value = useMemo(
    () =>
      settings && {
        settings,
        status,
        saveSettings,
        replaceSettings: setSettings,
        notify,
        attempt,
        toasts,
      },
    [attempt, notify, saveSettings, settings, status, toasts],
  );

  if (!value) return <>{fallback}</>;
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}
