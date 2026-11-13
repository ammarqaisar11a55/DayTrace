import { useCallback, useState } from "react";
import { ActivityBars, RankedList, ShareBar } from "../components/charts";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { DayRibbon } from "../components/DayRibbon";
import { SessionDialog } from "../components/SessionDialog";
import { Empty, ErrorNote, PageHeader, Section } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Session, Totals } from "../types";
import { addDays, formatClock, formatDate, formatDuration, sessionTitle, toDateKey } from "../utils/format";

function Figures({ totals }: { totals: Totals }) {
  const figures = [
    { label: "Active", value: totals.activeMs, primary: true },
    { label: "Idle", value: totals.idleMs },
    { label: "Computer use", value: totals.totalMs },
    { label: "Not recorded", value: totals.untrackedMs },
  ];
  return (
    <dl className="grid grid-cols-2 gap-y-4 sm:grid-cols-4">
      {figures.map((figure) => (
        <div key={figure.label} className="border-l border-line-strong pl-3 first:border-l-2 first:border-primary">
          <dt className="text-[13px] text-fg-2">{figure.label}</dt>
          <dd className={`leading-tight font-semibold tracking-tight ${figure.primary ? "text-[30px]" : "text-[24px]"}`}>
            {formatDuration(figure.value)}
          </dd>
        </div>
      ))}
    </dl>
  );
}

function RecentSessions({ sessions, onSelect }: { sessions: Session[]; onSelect: (session: Session) => void }) {
  if (sessions.length === 0) return <Empty>Sessions appear here as they are recorded.</Empty>;
  return (
    <ul className="-mx-1.5">
      {sessions.map((session) => (
        <li key={session.id}>
          <button
            type="button"
            onClick={() => onSelect(session)}
            className="flex w-full items-center gap-2.5 rounded px-1.5 py-1 text-left text-[13px] hover:bg-line/40"
          >
            <span className="w-[62px] shrink-0 whitespace-nowrap text-fg-3">{formatClock(session.startTime)}</span>
            <span className="min-w-0 flex-1 truncate">{sessionTitle(session)}</span>
            <span className="shrink-0 text-fg-2">{formatDuration(session.endTime - session.startTime)}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}

export function Dashboard() {
  const { settings } = useApp();
  const [selection, setSelection] = useRangeSelection("today");
  const { range } = selection;
  const [selected, setSelected] = useState<Session | null>(null);

  const report = useData(() => api.getReport(range), [range]);
  const sessions = useData(() => api.listSessions({ ...range }), [range]);
  const week = useData(() => {
    const today = toDateKey(new Date());
    return api.getReport({ from: addDays(today, -6), to: today });
  }, []);
  const recent = useData(() => api.listSessions({ ...range, limit: 8, newestFirst: true }), [range]);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const reload = () => {
    report.reload();
    sessions.reload();
    recent.reload();
  };

  const singleDay = range.from === range.to;
  const hourLabel = useCallback((key: string) => key, []);
  const dayLabel = useCallback((key: string) => formatDate(key, { weekday: "short", day: "numeric" }), []);
  const nothingYet = report.data?.totals.totalMs === 0;

  return (
    <>
      <PageHeader title="Dashboard">
        <DateRangePicker value={selection} onChange={setSelection} />
      </PageHeader>

      {report.error && <ErrorNote message={report.error} />}
      {nothingYet && !settings.trackingEnabled && (
        <p className="mb-4 rounded-md bg-warn-soft px-3 py-2 text-[13px] text-warn">
          Automatic tracking is off, so nothing is being recorded. Turn it on in{" "}
          <a href="#settings" className="underline">
            Settings
          </a>
          , or start a timer.
        </p>
      )}

      <div className="mb-7">
        <DayRibbon range={range} sessions={sessions.data ?? []} onSelect={setSelected} />
      </div>

      {report.data && (
        <>
          <div className="mb-8">
            <Figures totals={report.data.totals} />
          </div>

          <div className="grid gap-x-8 gap-y-7 lg:grid-cols-[3fr_2fr]">
            <Section title={singleDay ? "Activity by hour" : "Activity by day"}>
              <ActivityBars
                buckets={singleDay ? report.data.hourly : report.data.daily}
                label={singleDay ? hourLabel : dayLabel}
                perHour={singleDay}
                ariaLabel={singleDay ? "Active and idle time by hour" : "Active and idle time by day"}
              />
            </Section>
            <Section title="Categories">
              <ShareBar items={report.data.categories} empty="No categorized activity in this period." />
            </Section>
            <Section title="This week">
              {week.data && (
                <ActivityBars buckets={week.data.daily} label={dayLabel} height={170} ariaLabel="Active and idle time over the last seven days" />
              )}
            </Section>
            <Section title="Most used applications">
              <RankedList items={report.data.applications} empty="No applications recorded in this period." />
            </Section>
            <Section title="Recent sessions" className="lg:col-span-2">
              <RecentSessions sessions={recent.data ?? []} onSelect={setSelected} />
            </Section>
          </div>
        </>
      )}

      <SessionDialog
        session={selected}
        categories={categories}
        onClose={() => setSelected(null)}
        onChanged={reload}
      />
    </>
  );
}
