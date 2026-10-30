import { ChevronLeft, ChevronRight, Plus } from "lucide-react";
import { useMemo, useState } from "react";
import { AppIcon } from "../components/AppIcon";
import { DayRibbon } from "../components/DayRibbon";
import { CategoryOptions, ProjectOptions, SessionDialog } from "../components/SessionDialog";
import { Button, Dialog, Empty, ErrorNote, Field, IconButton, Input, PageHeader, Select } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Category, Project, Session } from "../types";
import {
  addDays,
  formatClock,
  formatDate,
  formatDuration,
  fromLocalInput,
  groupConsecutive,
  isAway,
  toDateKey,
  toLocalInput,
  type SessionGroup,
} from "../utils/format";

function AddEntryDialog({
  open,
  day,
  projects,
  categories,
  onClose,
  onAdded,
}: {
  open: boolean;
  day: string;
  projects: Project[];
  categories: Category[];
  onClose: () => void;
  onAdded: () => void;
}) {
  const { attempt, notify } = useApp();
  const noon = new Date(`${day}T12:00`).getTime();
  const [start, setStart] = useState(toLocalInput(noon));
  const [end, setEnd] = useState(toLocalInput(noon + 1_800_000));
  const [label, setLabel] = useState("");
  const [projectId, setProjectId] = useState("");
  const [categoryId, setCategoryId] = useState("");

  const add = async () => {
    const created = await attempt(() =>
      api.createManualSession({
        startTime: fromLocalInput(start),
        endTime: fromLocalInput(end),
        label: label.trim() || null,
        projectId: projectId ? Number(projectId) : null,
        categoryId: categoryId ? Number(categoryId) : null,
      }),
    );
    if (created) {
      notify("Entry added");
      onAdded();
      onClose();
    }
  };

  return (
    <Dialog title="Add an entry" open={open} onClose={onClose}>
      <div className="grid grid-cols-2 gap-3">
        <div className="col-span-2">
          <Field label="Label">
            <Input value={label} onChange={(event) => setLabel(event.target.value)} placeholder="Meeting with supervisor" />
          </Field>
        </div>
        <Field label="Started">
          <Input type="datetime-local" value={start} onChange={(event) => setStart(event.target.value)} />
        </Field>
        <Field label="Ended">
          <Input type="datetime-local" value={end} onChange={(event) => setEnd(event.target.value)} />
        </Field>
        <Field label="Project">
          <Select value={projectId} onChange={(event) => setProjectId(event.target.value)}>
            <ProjectOptions projects={projects} />
          </Select>
        </Field>
        <Field label="Category">
          <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
            <CategoryOptions categories={categories} />
          </Select>
        </Field>
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <Button onClick={onClose}>Cancel</Button>
        <Button tone="primary" onClick={() => void add()}>
          Add entry
        </Button>
      </div>
    </Dialog>
  );
}

function GroupRow({ group, onSelect }: { group: SessionGroup; onSelect: (session: Session) => void }) {
  const [expanded, setExpanded] = useState(false);
  const first = group.sessions[0]!;
  const away = isAway(first);
  const repeated = group.sessions.length > 1;

  return (
    <li>
      <button
        type="button"
        aria-expanded={repeated ? expanded : undefined}
        onClick={() => (repeated ? setExpanded(!expanded) : onSelect(first))}
        className="flex w-full items-center gap-3 rounded px-2 py-1.5 text-left hover:bg-rule/40"
      >
        <span className="w-[62px] shrink-0 text-xs whitespace-nowrap text-ink-3">{formatClock(group.startTime)}</span>
        <span
          aria-hidden
          className={`h-7 w-1 shrink-0 rounded-sm ${away ? "hatch-idle" : ""}`}
          style={{
            background: away
              ? undefined
              : first.sessionType === "unknown"
                ? "var(--idle)"
                : (first.categoryColor ?? "var(--pen)"),
          }}
        />
        {first.sessionType === "active" ? (
          <AppIcon id={first.applicationId} name={group.title} />
        ) : (
          <span className="size-5 shrink-0" />
        )}
        <span className="min-w-0 flex-1">
          <span className={`block truncate text-[13px] ${away ? "text-ink-3" : ""}`}>{group.title}</span>
          {(first.categoryName || first.projectName || first.label) && (
            <span className="block truncate text-xs text-ink-3">
              {[first.sessionType === "manual" ? null : first.label, first.categoryName, first.projectName]
                .filter(Boolean)
                .join(", ")}
            </span>
          )}
        </span>
        {repeated && <span className="text-xs text-ink-3">{group.sessions.length} sessions</span>}
        <span className="w-16 shrink-0 text-right text-[13px] text-ink-2">{formatDuration(group.totalMs)}</span>
      </button>
      {expanded && (
        <ul className="mb-1 ml-[86px] border-l border-rule pl-3">
          {group.sessions.map((session) => (
            <li key={session.id}>
              <button
                type="button"
                onClick={() => onSelect(session)}
                className="flex w-full justify-between rounded px-2 py-1 text-left text-xs text-ink-2 hover:bg-rule/40"
              >
                <span>
                  {formatClock(session.startTime)} to {formatClock(session.endTime)}
                </span>
                <span>{formatDuration(session.endTime - session.startTime)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function Timeline() {
  const today = toDateKey(new Date());
  const [day, setDay] = useState(today);
  const [applicationId, setApplicationId] = useState("");
  const [projectId, setProjectId] = useState("");
  const [selected, setSelected] = useState<Session | null>(null);
  const [adding, setAdding] = useState(false);

  const range = useMemo(() => ({ from: day, to: day }), [day]);
  const sessions = useData(
    () =>
      api.listSessions({
        ...range,
        applicationId: applicationId ? Number(applicationId) : null,
        projectId: projectId ? Number(projectId) : null,
      }),
    [range, applicationId, projectId],
  );
  const { data: applications = [] } = useData(() => api.listApplications(), []);
  const { data: projects = [] } = useData(() => api.listProjects(false), []);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const groups = useMemo(() => groupConsecutive(sessions.data ?? []), [sessions.data]);

  return (
    <>
      <PageHeader title="Timeline">
        <div className="flex items-center gap-1">
          <IconButton label="Previous day" onClick={() => setDay(addDays(day, -1))}>
            <ChevronLeft size={16} />
          </IconButton>
          <Input
            type="date"
            aria-label="Day"
            className="w-36"
            value={day}
            max={today}
            onChange={(event) => event.target.value && setDay(event.target.value)}
          />
          <IconButton label="Next day" disabled={day >= today} onClick={() => setDay(addDays(day, 1))}>
            <ChevronRight size={16} />
          </IconButton>
        </div>
        <Button tone="primary" onClick={() => setAdding(true)}>
          <Plus size={14} />
          Add entry
        </Button>
      </PageHeader>

      <div className="mb-5">
        <DayRibbon range={range} sessions={sessions.data ?? []} onSelect={setSelected} />
      </div>

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Select
          aria-label="Filter by application"
          className="w-48"
          value={applicationId}
          onChange={(event) => setApplicationId(event.target.value)}
        >
          <option value="">All applications</option>
          {applications.map((application) => (
            <option key={application.id} value={application.id}>
              {application.displayName ?? application.name}
            </option>
          ))}
        </Select>
        <Select
          aria-label="Filter by project"
          className="w-48"
          value={projectId}
          onChange={(event) => setProjectId(event.target.value)}
        >
          <option value="">All projects</option>
          {projects.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </Select>
      </div>

      {sessions.error && <ErrorNote message={sessions.error} />}
      {groups.length === 0 && !sessions.error && (
        <Empty>Nothing was recorded on {formatDate(day, { weekday: "long", month: "long", day: "numeric" })}.</Empty>
      )}
      <ol className="-mx-2">
        {groups.map((group) => (
          <GroupRow key={`${group.startTime}-${group.key}`} group={group} onSelect={setSelected} />
        ))}
      </ol>

      <SessionDialog
        session={selected}
        projects={projects}
        categories={categories}
        onClose={() => setSelected(null)}
        onChanged={sessions.reload}
      />
      <AddEntryDialog
        open={adding}
        day={day}
        projects={projects}
        categories={categories}
        onClose={() => setAdding(false)}
        onAdded={sessions.reload}
      />
    </>
  );
}
