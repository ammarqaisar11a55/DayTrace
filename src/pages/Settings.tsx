import { listen } from "@tauri-apps/api/event";
import { Trash2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { CapabilityList } from "../components/CapabilityList";
import { Button, ConfirmDialog, Empty, Field, IconButton, Input, PageHeader, Section, Select, Switch } from "../components/ui";
import { useData } from "../hooks/useData";
import { api, errorMessage } from "../services/api";
import { useApp } from "../stores/app";
import type { AvailableUpdate, ExclusionKind, Project, Settings, UpdateProgress } from "../types";
import { formatDateTime } from "../utils/format";

function Toggle({
  label,
  hint,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-6 py-2">
      <div>
        <div className="text-[13px]">{label}</div>
        {hint && <div className="max-w-prose text-xs text-ink-3">{hint}</div>}
      </div>
      <Switch checked={checked} onChange={onChange} label={label} disabled={disabled} />
    </div>
  );
}

function NumberField({
  label,
  hint,
  value,
  min,
  max,
  onCommit,
}: {
  label: string;
  hint: string;
  value: number;
  min: number;
  max: number;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  return (
    <Field label={label} hint={hint}>
      <Input
        type="number"
        min={min}
        max={max}
        className="w-28"
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => {
          const next = Math.min(max, Math.max(min, Number(draft) || min));
          setDraft(String(next));
          if (next !== value) onCommit(next);
        }}
      />
    </Field>
  );
}

function TrackingSettings() {
  const { settings, saveSettings, attempt, status } = useApp();
  const paused = status?.state === "paused";

  return (
    <Section title="Tracking">
      <Toggle
        label="Automatic tracking"
        hint="Record the focused application and idle time in the background."
        checked={settings.trackingEnabled}
        onChange={(trackingEnabled) => void saveSettings({ trackingEnabled })}
      />
      {settings.trackingEnabled && (
        <div className="flex items-center gap-2 py-2">
          {paused ? (
            <Button onClick={() => void attempt(() => api.resumeTracking())}>Resume tracking</Button>
          ) : (
            <>
              <Button onClick={() => void attempt(() => api.pauseTracking(30))}>Pause for 30 minutes</Button>
              <Button onClick={() => void attempt(() => api.pauseTracking(60))}>Pause for 1 hour</Button>
              <Button onClick={() => void attempt(() => api.pauseTracking(null))}>Pause until resumed</Button>
            </>
          )}
        </div>
      )}
      <div className="grid gap-4 py-2 sm:grid-cols-2">
        <NumberField
          label="Idle after (minutes)"
          hint="No input for this long counts as idle, not as use."
          value={Math.round(settings.idleThresholdSecs / 60)}
          min={1}
          max={60}
          onCommit={(minutes) => void saveSettings({ idleThresholdSecs: minutes * 60 })}
        />
        <NumberField
          label="Check every (seconds)"
          hint="Window switches are usually noticed immediately. This is the fallback."
          value={settings.pollIntervalSecs}
          min={1}
          max={60}
          onCommit={(pollIntervalSecs) => void saveSettings({ pollIntervalSecs })}
        />
      </div>
    </Section>
  );
}

function PrivacySettings() {
  const { settings, saveSettings } = useApp();
  return (
    <Section title="Privacy">
      <Toggle
        label="Collect window titles"
        hint="Titles can contain document names, email subjects and other private details. They are stored only on this computer."
        checked={settings.collectWindowTitles}
        onChange={(collectWindowTitles) => void saveSettings({ collectWindowTitles })}
      />
      <div className="py-2">
        <Field label="Keep history for" hint="Older sessions are deleted automatically.">
          <Select
            className="w-44"
            value={settings.retentionDays}
            onChange={(event) => void saveSettings({ retentionDays: Number(event.target.value) })}
          >
            <option value={0}>Forever</option>
            <option value={30}>30 days</option>
            <option value={90}>90 days</option>
            <option value={180}>6 months</option>
            <option value={365}>1 year</option>
            <option value={730}>2 years</option>
          </Select>
        </Field>
      </div>
    </Section>
  );
}

const KIND_LABELS: Record<ExclusionKind, string> = {
  application: "Application",
  path: "Program path",
  project: "Project",
};

function Exclusions() {
  const { attempt } = useApp();
  const exclusions = useData(() => api.listExclusions(), []);
  const { data: projects = [] } = useData<Project[]>(() => api.listProjects(false), []);
  const [kind, setKind] = useState<ExclusionKind>("application");
  const [value, setValue] = useState("");

  const add = () =>
    attempt(async () => {
      await api.addExclusion(kind, value);
      setValue("");
      exclusions.reload();
    });

  const describe = (exclusionKind: ExclusionKind, stored: string) =>
    exclusionKind === "project"
      ? (projects.find((project) => String(project.id) === stored)?.name ?? "Deleted project")
      : stored;

  return (
    <Section title="Never record">
      <p className="mb-3 text-xs text-ink-3">
        Time in these is left out of your history entirely. Adding one does not remove what is already recorded.
      </p>
      <div className="mb-3 flex gap-2">
        <Select
          aria-label="Type"
          className="w-36 shrink-0"
          value={kind}
          onChange={(event) => {
            setKind(event.target.value as ExclusionKind);
            setValue("");
          }}
        >
          {Object.entries(KIND_LABELS).map(([id, label]) => (
            <option key={id} value={id}>
              {label}
            </option>
          ))}
        </Select>
        {kind === "project" ? (
          <Select aria-label="Project" value={value} onChange={(event) => setValue(event.target.value)}>
            <option value="">Choose a project</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </Select>
        ) : (
          <Input
            aria-label="Value"
            value={value}
            placeholder={kind === "path" ? "/home/me/private" : "KeePassXC"}
            onChange={(event) => setValue(event.target.value)}
          />
        )}
        <Button disabled={!value.trim()} onClick={() => void add()}>
          Add
        </Button>
      </div>
      {exclusions.data?.length === 0 && <Empty>Nothing is excluded.</Empty>}
      <ul>
        {(exclusions.data ?? []).map((exclusion) => (
          <li key={exclusion.id} className="flex items-center gap-3 border-t border-rule py-1 text-[13px]">
            <span className="w-24 shrink-0 text-xs text-ink-3">{KIND_LABELS[exclusion.kind]}</span>
            <span className="selectable min-w-0 flex-1 truncate">{describe(exclusion.kind, exclusion.value)}</span>
            <IconButton
              label={`Stop excluding ${exclusion.value}`}
              onClick={() =>
                void attempt(async () => {
                  await api.removeExclusion(exclusion.id);
                  exclusions.reload();
                })
              }
            >
              <Trash2 size={14} />
            </IconButton>
          </li>
        ))}
      </ul>
    </Section>
  );
}

function DataSettings() {
  const { attempt, notify } = useApp();
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [confirmingRestore, setConfirmingRestore] = useState(false);

  const exportAs = async (format: "json" | "csv") => {
    const path = await attempt(() => api.exportData(format));
    if (path) notify(`Exported to ${path}`);
  };

  const backup = async () => {
    const path = await attempt(() => api.backupDatabase());
    if (path) notify(`Backup saved to ${path}`);
  };

  const restore = async () => {
    setConfirmingRestore(false);
    const path = await attempt(() => api.restoreDatabase());
    if (path) notify(`Restored. Your previous data was kept at ${path}`);
  };

  const removeAll = async () => {
    setConfirmingDelete(false);
    const count = await attempt(() => api.deleteAllActivity());
    if (count !== undefined) notify(`${count} sessions deleted`);
  };

  return (
    <Section title="Your data">
      <div className="flex flex-wrap gap-2 py-1">
        <Button onClick={() => void exportAs("json")}>Export as JSON</Button>
        <Button onClick={() => void exportAs("csv")}>Export as CSV</Button>
        <Button onClick={() => void backup()}>Back up database</Button>
        <Button onClick={() => setConfirmingRestore(true)}>Restore from backup</Button>
        <Button tone="danger" onClick={() => setConfirmingDelete(true)}>
          Delete all history
        </Button>
      </div>
      <ConfirmDialog
        open={confirmingDelete}
        title="Delete all activity history?"
        body="Every recorded session is removed from this computer. Your projects, categories and rules stay. This cannot be undone."
        action="Delete all history"
        onConfirm={() => void removeAll()}
        onClose={() => setConfirmingDelete(false)}
      />
      <ConfirmDialog
        open={confirmingRestore}
        title="Replace your data with a backup?"
        body="Everything now in DayTrace is replaced by the backup you choose. A copy of your current data is kept beside the database."
        action="Choose backup"
        onConfirm={() => void restore()}
        onClose={() => setConfirmingRestore(false)}
      />
    </Section>
  );
}

function GeneralSettings() {
  const { settings, saveSettings } = useApp();
  return (
    <Section title="General">
      <div className="py-2">
        <Field label="Theme">
          <Select
            className="w-44"
            value={settings.theme}
            onChange={(event) => void saveSettings({ theme: event.target.value as Settings["theme"] })}
          >
            <option value="system">Match system</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </Select>
        </Field>
      </div>
    </Section>
  );
}

function StartupSettings() {
  const { settings, saveSettings } = useApp();
  return (
    <Section title="Startup and background">
      <Toggle
        label="Start automatically when you turn on your computer"
        hint="DayTrace opens each time you sign in to this computer."
        checked={settings.startOnLogin}
        onChange={(startOnLogin) => void saveSettings({ startOnLogin })}
      />
      <Toggle
        label="Run in the background"
        hint={
          settings.runInBackground
            ? "Closing the window keeps DayTrace running, so tracking continues."
            : "Closing the window quits DayTrace, and tracking stops until you open it again."
        }
        checked={settings.runInBackground}
        onChange={(runInBackground) => void saveSettings({ runInBackground })}
      />
      <Toggle
        label="Start without opening a window"
        hint={
          settings.startOnLogin && settings.runInBackground
            ? "At startup, DayTrace begins tracking without showing its window."
            : "Needs both options above to be on."
        }
        checked={settings.startHidden}
        disabled={!settings.startOnLogin || !settings.runInBackground}
        onChange={(startHidden) => void saveSettings({ startHidden })}
      />
      <div className="py-2">
        <Button onClick={() => void api.quitApp()}>Quit DayTrace</Button>
      </div>
    </Section>
  );
}

type UpdateState =
  | { name: "idle" }
  | { name: "checking" }
  | { name: "latest" }
  | { name: "available"; update: AvailableUpdate }
  | { name: "downloading"; update: AvailableUpdate; percent: number | null }
  | { name: "verifying"; update: AvailableUpdate }
  | { name: "ready"; update: AvailableUpdate }
  | { name: "installing"; update: AvailableUpdate }
  | { name: "restart"; update: AvailableUpdate }
  | { name: "failed"; message: string };

function UpdateSettings() {
  const { settings, saveSettings } = useApp();
  const { data: info } = useData(() => api.updateInfo(), [settings.updateChannel]);
  const [state, setState] = useState<UpdateState>({ name: "idle" });

  useEffect(() => {
    const unlisten = listen<UpdateProgress>("update-progress", ({ payload }) => {
      setState((current) => {
        if (current.name !== "downloading") return current;
        const percent = payload.total ? Math.round((payload.downloaded / payload.total) * 100) : null;
        return percent === 100 ? { name: "verifying", update: current.update } : { ...current, percent };
      });
    });
    return () => void unlisten.then((stop) => stop());
  }, []);

  const check = async () => {
    setState({ name: "checking" });
    try {
      const update = await api.checkForUpdate();
      setState(update ? { name: "available", update } : { name: "latest" });
    } catch (error) {
      setState({ name: "failed", message: errorMessage(error) });
    }
  };

  const download = async (update: AvailableUpdate) => {
    setState({ name: "downloading", update, percent: 0 });
    try {
      await api.downloadUpdate();
      setState({ name: "ready", update });
    } catch (error) {
      setState({ name: "failed", message: errorMessage(error) });
    }
  };

  const install = async (update: AvailableUpdate) => {
    setState({ name: "installing", update });
    try {
      await api.installUpdate();
      setState({ name: "restart", update });
    } catch (error) {
      setState({ name: "failed", message: errorMessage(error) });
    }
  };

  const update = "update" in state ? state.update : null;

  return (
    <Section title="Updates">
      <div className="mb-2 text-[13px]">
        <p>Version {info?.currentVersion}</p>
        {settings.lastUpdateCheck && (
          <p className="text-xs text-ink-3">Last checked {formatDateTime(settings.lastUpdateCheck)}</p>
        )}
      </div>

      <div className="mb-3 flex min-h-8 flex-wrap items-center gap-3" aria-live="polite">
        {(state.name === "idle" || state.name === "latest" || state.name === "failed") && (
          <Button onClick={() => void check()}>Check for updates</Button>
        )}
        {state.name === "checking" && <span className="text-[13px]">Checking for updates...</span>}
        {state.name === "latest" && <span className="text-[13px]">You are using the latest version.</span>}
        {state.name === "failed" && <span className="text-[13px] text-danger">{state.message}</span>}
        {state.name === "available" && (
          <>
            <span className="text-[13px] font-medium">Update available: v{state.update.version}</span>
            {info?.canInstall && (
              <Button tone="primary" onClick={() => void download(state.update)}>
                Download update
              </Button>
            )}
          </>
        )}
        {state.name === "downloading" && (
          <div className="w-64">
            <div className="mb-1 text-[13px]">Downloading update...{state.percent !== null ? ` ${state.percent}%` : ""}</div>
            <div className="h-1.5 bg-rule">
              <div className="h-full bg-pen" style={{ width: `${state.percent ?? 0}%` }} />
            </div>
          </div>
        )}
        {state.name === "verifying" && <span className="text-[13px]">Verifying update...</span>}
        {state.name === "ready" && (
          <>
            <span className="text-[13px]">Update ready to install.</span>
            <Button tone="primary" onClick={() => void install(state.update)}>
              Install update
            </Button>
          </>
        )}
        {state.name === "installing" && <span className="text-[13px]">Installing update...</span>}
        {state.name === "restart" && (
          <>
            <span className="text-[13px]">Restart to complete update.</span>
            <Button tone="primary" onClick={() => void api.restartApp()}>
              Restart now
            </Button>
          </>
        )}
      </div>

      {update?.notes && (
        <pre className="selectable mb-3 max-h-40 overflow-y-auto rounded-md bg-rule/40 p-3 font-sans text-xs whitespace-pre-wrap">
          {update.notes}
        </pre>
      )}
      {info?.guidance && <p className="mb-3 max-w-prose text-xs text-ink-2">{info.guidance}</p>}

      <div className="py-2">
        <Field label="Update channel" hint="Beta versions arrive sooner and may be less stable.">
          <Select
            className="w-44"
            value={settings.updateChannel}
            onChange={(event) =>
              void saveSettings({ updateChannel: event.target.value as Settings["updateChannel"] })
            }
          >
            <option value="stable">Stable</option>
            <option value="beta">Beta</option>
          </Select>
        </Field>
      </div>
      <Toggle
        label="Check when DayTrace starts"
        checked={settings.updateCheckOnStartup}
        onChange={(updateCheckOnStartup) => void saveSettings({ updateCheckOnStartup })}
      />
      <div className="py-2">
        <NumberField
          label="Check every (hours)"
          hint="Only your version and platform are visible to GitHub when checking."
          value={settings.updateCheckIntervalHours}
          min={6}
          max={168}
          onCommit={(updateCheckIntervalHours) => void saveSettings({ updateCheckIntervalHours })}
        />
      </div>
    </Section>
  );
}

const TABS = [
  { id: "tracking", label: "Tracking" },
  { id: "privacy", label: "Privacy and data" },
  { id: "general", label: "General" },
  { id: "updates", label: "Updates" },
] as const;

type TabId = (typeof TABS)[number]["id"];

function Panel({ children }: { children: ReactNode }) {
  return <div className="max-w-2xl space-y-7">{children}</div>;
}

export function SettingsPage() {
  const [tab, setTab] = useState<TabId>("tracking");
  return (
    <>
      <PageHeader title="Settings" />
      <div role="tablist" aria-label="Settings sections" className="mb-6 flex gap-5 border-b border-rule">
        {TABS.map(({ id, label }) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            onClick={() => setTab(id)}
            className={`-mb-px border-b-2 pb-2 text-[13px] transition-colors ${
              tab === id ? "border-pen font-medium text-ink" : "border-transparent text-ink-2 hover:text-ink"
            }`}
          >
            {label}
          </button>
        ))}
      </div>
      {tab === "tracking" && (
        <Panel>
          <TrackingSettings />
          <Section title="What works on this computer">
            <CapabilityList />
          </Section>
        </Panel>
      )}
      {tab === "privacy" && (
        <Panel>
          <PrivacySettings />
          <Exclusions />
          <DataSettings />
        </Panel>
      )}
      {tab === "general" && (
        <Panel>
          <StartupSettings />
          <GeneralSettings />
        </Panel>
      )}
      {tab === "updates" && (
        <Panel>
          <UpdateSettings />
        </Panel>
      )}
    </>
  );
}
