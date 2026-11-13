import { CircleSlash, Clock, Hourglass, MonitorSmartphone, Pause, Play } from "lucide-react";
import { useCallback, useState } from "react";
import { ActiveIdleSplit, ActivityBars, RankedList, ShareBar } from "../components/charts";
import { RecentActivity } from "../components/dashboard/RecentActivity";
import { StatCard } from "../components/dashboard/StatCard";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { DayRibbon } from "../components/DayRibbon";
import { SessionDialog } from "../components/SessionDialog";
import { Button, ErrorNote, PageHeader, Section } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Session, Totals } from "../types";
import { addDays, daysInRange, formatClock, formatDate, formatDuration, toDateKey } from "../utils/format";
import { describe } from "../utils/status";

function TrackingControl() {
  const { status, settings, attempt } = useApp();
  const summary = describe(status);
  const state = status?.state;
  return (
    <div className="flex items-center gap-2">
      <span className="inline-flex h-9 max-w-72 items-center gap-2 rounded-md border border-line bg-surface px-3 text-[13px]">
        <span aria-hidden className={`size-2 shrink-0 rounded-full ${summary.live ? "live-pulse bg-brand" : "bg-line-strong"}`} />
        <span className="font-medium">{summary.title}</span>
        {summary.detail && <span className="truncate text-fg-3">{summary.detail}</span>}
      </span>
      {settings.trackingEnabled && state !== "paused" && state !== "manual" && (
        <Button onClick={() => void attempt(() => api.pauseTracking(null))}>
          <Pause size={14} />
          Pause tracking
        </Button>
      )}
      {state === "paused" && (
        <Button tone="primary" onClick={() => void attempt(() => api.resumeTracking())}>
          <Play size={14} />
          Resume tracking
        </Button>
      )}
    </div>
  );
}

function share(part: number, whole: number): string {
  return whole > 0 ? `${Math.round((part / whole) * 100)}% of computer use` : "Nothing recorded yet";
}

function Figures({ totals, sessions, days }: { totals: Totals; sessions: Session[]; days: number }) {
  const first = sessions[0];
  const last = sessions.at(-1);
  const span =
    days === 1
      ? first && last
        ? `${formatClock(first.startTime)} to ${formatClock(last.endTime)}`
        : "No sessions yet"
      : `${formatDuration(totals.totalMs / days)} a day on average`;
  return (
    <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard icon={Clock} label="Active" value={formatDuration(totals.activeMs)} meta={share(totals.activeMs, totals.totalMs)} />
      <StatCard icon={Hourglass} label="Idle" value={formatDuration(totals.idleMs)} meta={share(totals.idleMs, totals.totalMs)} />
      <StatCard icon={MonitorSmartphone} label="Computer use" value={formatDuration(totals.totalMs)} meta={span} />
      <StatCard icon={CircleSlash} label="Not recorded" value={formatDuration(totals.untrackedMs)} meta="Time with no sessions" />
    </div>
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
  const today = formatDate(toDateKey(new Date()), { weekday: "long", month: "long", day: "numeric" });

  return (
    <>
      <PageHeader title="Dashboard" description={today}>
        <TrackingControl />
      </PageHeader>

      <div className="mb-5">
        <DateRangePicker value={selection} onChange={setSelection} />
      </div>

      {report.error && <ErrorNote message={report.error} />}
      {nothingYet && !settings.trackingEnabled && (
        <p className="mb-5 rounded-md border border-warn/30 bg-warn-soft px-3.5 py-2.5 text-[13px] text-warn">
          Automatic tracking is off, so nothing is being recorded. Turn it on in{" "}
          <a href="#settings" className="font-medium underline underline-offset-2">
            Settings
          </a>
          , or start a timer.
        </p>
      )}

      {report.data && (
        <div className="space-y-5">
          <Figures totals={report.data.totals} sessions={sessions.data ?? []} days={daysInRange(range).length} />

          <Section
            title={singleDay ? "Your day" : "Day by day"}
            description={singleDay ? "Midnight to midnight. Select a segment to edit it." : "One row per day, midnight to midnight."}
          >
            <DayRibbon range={range} sessions={sessions.data ?? []} onSelect={setSelected} />
          </Section>

          <div className="grid gap-5 lg:grid-cols-[3fr_2fr]">
            <Section title={singleDay ? "Activity by hour" : "Activity by day"}>
              <ActivityBars
                buckets={singleDay ? report.data.hourly : report.data.daily}
                label={singleDay ? hourLabel : dayLabel}
                perHour={singleDay}
                ariaLabel={singleDay ? "Active and idle time by hour" : "Active and idle time by day"}
              />
            </Section>
            <Section title="Active and idle">
              <ActiveIdleSplit totals={report.data.totals} />
            </Section>
            <Section title="Most used applications">
              <RankedList items={report.data.applications} empty="No applications recorded in this period." />
            </Section>
            <Section title="Categories">
              <ShareBar items={report.data.categories} empty="No categorized activity in this period." />
            </Section>
            <Section title="Recent activity">
              <RecentActivity sessions={recent.data ?? []} onSelect={setSelected} />
            </Section>
            <Section title="Last 7 days">
              {week.data && (
                <ActivityBars buckets={week.data.daily} label={dayLabel} height={190} ariaLabel="Active and idle time over the last seven days" />
              )}
            </Section>
          </div>
        </div>
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
