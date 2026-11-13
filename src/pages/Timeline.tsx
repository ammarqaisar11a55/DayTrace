import { CalendarX2, ChevronLeft, ChevronRight, Plus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { AppIcon } from "../components/AppIcon";
import { DayRibbon } from "../components/DayRibbon";
import { CategoryOptions, SessionDialog } from "../components/SessionDialog";
import { Button, Dialog, Empty, ErrorNote, Field, IconButton, Input, PageHeader, Section, Select } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Category, Session } from "../types";
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
  categories,
  onClose,
  onAdded,
}: {
  open: boolean;
  day: string;
  categories: Category[];
  onClose: () => void;
  onAdded: () => void;
}) {
  const { attempt, notify } = useApp();
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [label, setLabel] = useState("");
  const [categoryId, setCategoryId] = useState("");

  useEffect(() => {
    if (!open) return;
    // Noon works for any past day, but on today it can still be in the future.
    const defaultEnd = Math.min(new Date(`${day}T12:30`).getTime(), Date.now());
    setStart(toLocalInput(defaultEnd - 1_800_000));
    setEnd(toLocalInput(defaultEnd));
  }, [open, day]);

  const add = async () => {
    const created = await attempt(() =>
      api.createManualSession({
        startTime: fromLocalInput(start),
        endTime: fromLocalInput(end),
        label: label.trim() || null,
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
        <div className="col-span-2">
          <Field label="Category">
            <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
              <CategoryOptions categories={categories} />
            </Select>
          </Field>
        </div>
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
        className="flex w-full items-center gap-3 rounded-sm px-2 py-2 text-left transition-colors hover:bg-hover"
      >
        <span className="w-[62px] shrink-0 text-xs whitespace-nowrap text-fg-3">{formatClock(group.startTime)}</span>
        <span
          aria-hidden
          className={`h-8 w-1 shrink-0 rounded-full ${away ? "hatch-idle" : ""}`}
          style={{
            background: away
              ? undefined
              : first.sessionType === "unknown"
                ? "var(--dt-idle)"
                : (first.categoryColor ?? "var(--dt-primary)"),
          }}
        />
        {first.sessionType === "active" ? (
          <AppIcon id={first.applicationId} name={group.title} />
        ) : (
          <span className="size-5 shrink-0" />
        )}
        <span className="min-w-0 flex-1">
          <span className={`block truncate text-[13px] ${away ? "text-fg-3" : ""}`}>{group.title}</span>
          {(first.categoryName || first.label) && (
            <span className="block truncate text-xs text-fg-3">
              {[first.sessionType === "manual" ? null : first.label, first.categoryName]
                .filter(Boolean)
                .join(", ")}
            </span>
          )}
        </span>
        {repeated && (
          <span className="rounded-full bg-hover px-2 py-0.5 text-[11px] text-fg-2">{group.sessions.length} sessions</span>
        )}
        <span className="w-16 shrink-0 text-right text-[13px] text-fg-2">{formatDuration(group.totalMs)}</span>
      </button>
      {expanded && (
        <ul className="mb-1 ml-[88px] border-l border-line pl-3">
          {group.sessions.map((session) => (
            <li key={session.id}>
              <button
                type="button"
                onClick={() => onSelect(session)}
                className="flex w-full justify-between rounded-sm px-2 py-1.5 text-left text-xs text-fg-2 transition-colors hover:bg-hover"
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
  const [selected, setSelected] = useState<Session | null>(null);
  const [adding, setAdding] = useState(false);

  const range = useMemo(() => ({ from: day, to: day }), [day]);
  const sessions = useData(
    () =>
      api.listSessions({
        ...range,
        applicationId: applicationId ? Number(applicationId) : null,
      }),
    [range, applicationId],
  );
  const { data: applications = [] } = useData(() => api.listApplications(), []);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const groups = useMemo(() => groupConsecutive(sessions.data ?? []), [sessions.data]);

  const yesterday = addDays(today, -1);
  const longDay = formatDate(day, { weekday: "long", month: "long", day: "numeric", year: "numeric" });

  return (
    <>
      <PageHeader title="Timeline" description={longDay}>
        <div role="group" aria-label="Quick days" className="flex h-9 items-center gap-0.5 rounded-md border border-line bg-surface p-1">
          {[
            { key: today, label: "Today" },
            { key: yesterday, label: "Yesterday" },
          ].map((option) => (
            <button
              key={option.label}
              type="button"
              aria-pressed={day === option.key}
              onClick={() => setDay(option.key)}
              className={`h-full rounded-sm px-3 text-[13px] transition-colors duration-150 ${
                day === option.key ? "bg-primary-soft font-medium text-accent" : "text-fg-2 hover:bg-hover hover:text-fg"
              }`}
            >
              {option.label}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-1">
          <IconButton label="Previous day" onClick={() => setDay(addDays(day, -1))}>
            <ChevronLeft size={16} />
          </IconButton>
          <Input
            type="date"
            aria-label="Day"
            className="w-40"
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

      <div className="space-y-5">
        <Section title="Day at a glance" description="Hover a segment for details, or select it to edit.">
          <DayRibbon range={range} sessions={sessions.data ?? []} onSelect={setSelected} />
        </Section>

        <Section
          title="Sessions"
          description={groups.length > 0 ? `${groups.length} entries, repeated switches grouped` : undefined}
          actions={
            <Select
              aria-label="Filter by application"
              className="w-52"
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
          }
        >
          {sessions.error && <ErrorNote message={sessions.error} />}
          {groups.length === 0 && !sessions.error && (
            <Empty icon={CalendarX2}>
              Nothing was recorded on {formatDate(day, { weekday: "long", month: "long", day: "numeric" })}.
            </Empty>
          )}
          <ol className="-mx-2">
            {groups.map((group) => (
              <GroupRow key={`${group.startTime}-${group.key}`} group={group} onSelect={setSelected} />
            ))}
          </ol>
        </Section>
      </div>

      <SessionDialog
        session={selected}
        categories={categories}
        onClose={() => setSelected(null)}
        onChanged={sessions.reload}
      />
      <AddEntryDialog
        open={adding}
        day={day}
        categories={categories}
        onClose={() => setAdding(false)}
        onAdded={sessions.reload}
      />
    </>
  );
}
