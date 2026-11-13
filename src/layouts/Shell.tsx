import {
  AppWindow,
  ChartColumn,
  GanttChart,
  LayoutDashboard,
  Pause,
  Play,
  Settings,
  Square,
  Tags,
  Timer,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { CategoryOptions } from "../components/SessionDialog";
import { Button, Dialog, Field, Input, Select } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import { formatDuration } from "../utils/format";
import { describe } from "../utils/status";

export type PageId = "dashboard" | "timeline" | "applications" | "categories" | "analytics" | "settings";

interface NavigationEntry {
  id: PageId;
  label: string;
  icon: typeof Settings;
}

const NAVIGATION: NavigationEntry[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "timeline", label: "Timeline", icon: GanttChart },
  { id: "applications", label: "Applications", icon: AppWindow },
  { id: "categories", label: "Categories", icon: Tags },
  { id: "analytics", label: "Analytics", icon: ChartColumn },
];

const SETTINGS: NavigationEntry = { id: "settings", label: "Settings", icon: Settings };

function useElapsed(since: number | null): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (since === null) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [since]);
  return since === null ? 0 : now - since;
}

function ManualTimerDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { attempt } = useApp();
  const [label, setLabel] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const { data: categories = [] } = useData(() => api.listCategories(), [open]);

  const start = async () => {
    await attempt(() =>
      api.startManualTimer({
        label: label.trim() || null,
        categoryId: categoryId ? Number(categoryId) : null,
      }),
    );
    setLabel("");
    onClose();
  };

  return (
    <Dialog title="Start a timer" open={open} onClose={onClose}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void start();
        }}
        className="space-y-3"
      >
        <Field label="What are you doing?">
          <Input value={label} autoFocus onChange={(event) => setLabel(event.target.value)} placeholder="Reading chapter 4" />
        </Field>
        <Field label="Category">
          <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
            <CategoryOptions categories={categories} />
          </Select>
        </Field>
        <p className="text-xs text-fg-3">
          A timer replaces automatic tracking and keeps counting while you are away from the keyboard.
        </p>
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button tone="primary" type="submit">
            Start timer
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function StatusPanel() {
  const { status, settings, attempt } = useApp();
  const [timerOpen, setTimerOpen] = useState(false);
  const elapsed = useElapsed(status?.manualStartedAt ?? null);
  const summary = describe(status);
  const state = status?.state;
  const canPause = settings.trackingEnabled && state !== "paused" && state !== "manual";

  return (
    <div className="rounded-md border border-line bg-surface p-3">
      <div className="mb-3 flex items-start gap-2.5" aria-live="polite">
        <span
          aria-hidden
          className={`mt-[5px] size-2 shrink-0 rounded-full ${summary.live ? "live-pulse bg-brand" : "bg-line-strong"}`}
        />
        <div className="min-w-0">
          <div className="text-[13px] leading-tight font-medium">{summary.title}</div>
          {summary.detail && <div className="mt-0.5 truncate text-xs text-fg-3">{summary.detail}</div>}
          {state === "manual" && <div className="mt-0.5 text-xs text-accent">{formatDuration(elapsed)}</div>}
        </div>
      </div>
      <div className="flex gap-1.5">
        {state === "manual" ? (
          <Button className="h-8 flex-1 px-2" onClick={() => void attempt(() => api.stopManualTimer())}>
            <Square size={13} />
            Stop timer
          </Button>
        ) : (
          <Button className="h-8 flex-1 px-2" onClick={() => setTimerOpen(true)}>
            <Timer size={14} />
            Timer
          </Button>
        )}
        {canPause && (
          <Button className="h-8 flex-1 px-2" onClick={() => void attempt(() => api.pauseTracking(null))}>
            <Pause size={13} />
            Pause
          </Button>
        )}
        {state === "paused" && (
          <Button className="h-8 flex-1 px-2" onClick={() => void attempt(() => api.resumeTracking())}>
            <Play size={13} />
            Resume
          </Button>
        )}
      </div>
      <ManualTimerDialog open={timerOpen} onClose={() => setTimerOpen(false)} />
    </div>
  );
}

function NavigationItem({ entry, current }: { entry: NavigationEntry; current: boolean }) {
  const { id, label, icon: Icon } = entry;
  return (
    <a
      href={`#${id}`}
      aria-current={current ? "page" : undefined}
      className={`relative flex h-9 items-center gap-3 rounded-sm px-3 text-[13px] transition-colors duration-150 ${
        current ? "bg-hover font-medium text-fg" : "text-fg-2 hover:bg-hover/60 hover:text-fg"
      }`}
    >
      {current && <span aria-hidden className="absolute inset-y-2 left-0 w-[3px] rounded-full bg-brand" />}
      <Icon size={16} strokeWidth={1.75} className={current ? "text-accent" : undefined} />
      {label}
    </a>
  );
}

function Toasts() {
  const { toasts } = useApp();
  return (
    <div className="pointer-events-none fixed right-5 bottom-5 z-30 space-y-2" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`max-w-sm rounded-md border px-3.5 py-2.5 text-[13px] shadow-md ${
            toast.tone === "error"
              ? "border-danger/40 bg-danger-soft text-danger"
              : "border-line bg-elevated text-fg"
          }`}
        >
          {toast.message}
        </div>
      ))}
    </div>
  );
}

export function Shell({ page, children }: { page: PageId; children: ReactNode }) {
  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-sidebar">
        <nav aria-label="Main" className="flex flex-1 flex-col gap-0.5 overflow-y-auto px-3 pt-4">
          {NAVIGATION.map((entry) => (
            <NavigationItem key={entry.id} entry={entry} current={page === entry.id} />
          ))}
          <div className="mt-auto pt-4 pb-2">
            <NavigationItem entry={SETTINGS} current={page === "settings"} />
          </div>
        </nav>
        <div className="px-3 pb-3">
          <StatusPanel />
        </div>
      </aside>
      {/* Positioned, so absolutely placed content such as screen-reader captions
          stays inside the scrolling area instead of stretching the whole page. */}
      <main className="relative min-w-0 flex-1 overflow-y-auto bg-app">
        <div className="mx-auto max-w-[1180px] px-8 py-7">{children}</div>
      </main>
      <Toasts />
    </div>
  );
}
