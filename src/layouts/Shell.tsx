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
import type { TrackerStatus } from "../types";
import { formatDuration, formatClock } from "../utils/format";

export type PageId = "dashboard" | "timeline" | "applications" | "categories" | "analytics" | "settings";

const NAVIGATION: { id: PageId; label: string; icon: typeof Settings }[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "timeline", label: "Timeline", icon: GanttChart },
  { id: "applications", label: "Applications", icon: AppWindow },
  { id: "categories", label: "Categories", icon: Tags },
  { id: "analytics", label: "Analytics", icon: ChartColumn },
  { id: "settings", label: "Settings", icon: Settings },
];

function describe(status: TrackerStatus | null): { title: string; detail?: string; live: boolean } {
  switch (status?.state) {
    case "tracking":
      return {
        title: "Tracking",
        detail: status.currentApplication ?? "Application not identified",
        live: true,
      };
    case "idle":
      return { title: "Idle", detail: "Waiting for activity", live: false };
    case "locked":
      return { title: "Locked", live: false };
    case "paused":
      return {
        title: "Paused",
        detail: status.pausedUntil ? `Until ${formatClock(status.pausedUntil)}` : "Until you resume",
        live: false,
      };
    case "manual":
      return { title: "Timer running", detail: status.manualLabel ?? "Manual entry", live: true };
    default:
      return { title: "Tracking is off", detail: "Turn it on in Settings", live: false };
  }
}

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
    <div className="border-t border-line px-3 py-3">
      <div className="mb-2.5 flex items-start gap-2 px-1" aria-live="polite">
        <span
          aria-hidden
          className={`mt-1.5 size-2 shrink-0 rounded-full ${summary.live ? "bg-primary" : "bg-line-strong"}`}
        />
        <div className="min-w-0">
          <div className="text-[13px] leading-tight font-medium">{summary.title}</div>
          {summary.detail && <div className="truncate text-xs text-fg-3">{summary.detail}</div>}
          {state === "manual" && <div className="text-xs text-fg-2">{formatDuration(elapsed)}</div>}
        </div>
      </div>
      <div className="flex gap-1.5">
        {state === "manual" ? (
          <Button className="flex-1" onClick={() => void attempt(() => api.stopManualTimer())}>
            <Square size={13} />
            Stop timer
          </Button>
        ) : (
          <Button className="flex-1" onClick={() => setTimerOpen(true)}>
            <Timer size={14} />
            Timer
          </Button>
        )}
        {canPause && (
          <Button className="flex-1" onClick={() => void attempt(() => api.pauseTracking(null))}>
            <Pause size={13} />
            Pause
          </Button>
        )}
        {state === "paused" && (
          <Button className="flex-1" onClick={() => void attempt(() => api.resumeTracking())}>
            <Play size={13} />
            Resume
          </Button>
        )}
      </div>
      <ManualTimerDialog open={timerOpen} onClose={() => setTimerOpen(false)} />
    </div>
  );
}

function Toasts() {
  const { toasts } = useApp();
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-30 space-y-2" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`max-w-sm rounded-md border px-3 py-2 text-[13px] ${
            toast.tone === "error"
              ? "border-danger bg-danger-soft text-danger"
              : "border-line-strong bg-surface text-fg"
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
      <aside className="flex w-52 shrink-0 flex-col border-r border-line">
        <div className="px-4 pt-4 pb-3">
          <span className="text-[17px] font-semibold tracking-tight">DayTrace</span>
        </div>
        <nav aria-label="Main" className="flex-1 space-y-0.5 overflow-y-auto px-2">
          {NAVIGATION.map(({ id, label, icon: Icon }) => (
            <a
              key={id}
              href={`#${id}`}
              aria-current={page === id ? "page" : undefined}
              className={`flex h-8 items-center gap-2.5 rounded-md px-2.5 text-[13px] transition-colors ${
                page === id ? "bg-primary-soft font-medium text-fg" : "text-fg-2 hover:bg-line/50 hover:text-fg"
              }`}
            >
              <Icon size={15} strokeWidth={1.75} />
              {label}
            </a>
          ))}
        </nav>
        <StatusPanel />
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-[1080px] px-7 py-6">{children}</div>
      </main>
      <Toasts />
    </div>
  );
}
