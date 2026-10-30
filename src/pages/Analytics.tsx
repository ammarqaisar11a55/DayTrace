import { useCallback, useState } from "react";
import { ActivityBars, HourHeatmap, RankedList, ShareBar, TrendLine } from "../components/charts";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { ColorSwatch, Empty, ErrorNote, PageHeader, Section } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import type { Bucket, Totals } from "../types";
import { formatDate, formatDuration, parseDateKey } from "../utils/format";

function ActiveIdleSplit({ totals }: { totals: Totals }) {
  if (totals.totalMs === 0) return <Empty>Nothing was recorded in this period.</Empty>;
  const activeShare = Math.round((totals.activeMs / totals.totalMs) * 100);
  return (
    <div>
      <div className="flex h-3 gap-0.5" role="img" aria-label={`${activeShare}% active, ${100 - activeShare}% idle`}>
        <div className="rounded-l-[3px] bg-pen" style={{ flexGrow: totals.activeMs, flexBasis: 0 }} />
        <div className="hatch-idle rounded-r-[3px]" style={{ flexGrow: totals.idleMs, flexBasis: 0 }} />
      </div>
      <dl className="mt-3 space-y-1 text-[13px]">
        <div className="flex justify-between">
          <dt className="text-ink-2">Active</dt>
          <dd>{formatDuration(totals.activeMs)}</dd>
        </div>
        <div className="flex justify-between">
          <dt className="text-ink-2">Idle or locked</dt>
          <dd>{formatDuration(totals.idleMs)}</dd>
        </div>
        {totals.unknownMs > 0 && (
          <div className="flex justify-between">
            <dt className="text-ink-2">Active, application unidentified</dt>
            <dd>{formatDuration(totals.unknownMs)}</dd>
          </div>
        )}
        {totals.manualMs > 0 && (
          <div className="flex justify-between">
            <dt className="text-ink-2">Timers and manual entries</dt>
            <dd>{formatDuration(totals.manualMs)}</dd>
          </div>
        )}
        <div className="flex justify-between">
          <dt className="text-ink-2">Not recorded</dt>
          <dd>{formatDuration(totals.untrackedMs)}</dd>
        </div>
      </dl>
    </div>
  );
}

function MonthlyTable({ months }: { months: Bucket[] }) {
  return (
    <table className="w-full text-left text-[13px]">
      <thead className="text-xs text-ink-3">
        <tr>
          <th scope="col" className="pb-1.5 font-medium">
            Month
          </th>
          <th scope="col" className="pb-1.5 text-right font-medium">
            Active
          </th>
          <th scope="col" className="pb-1.5 text-right font-medium">
            Idle
          </th>
        </tr>
      </thead>
      <tbody>
        {months.map((month) => (
          <tr key={month.key} className="border-t border-rule">
            <td className="py-1.5">
              {parseDateKey(`${month.key}-01`).toLocaleDateString(undefined, { month: "long", year: "numeric" })}
            </td>
            <td className="py-1.5 text-right">{formatDuration(month.activeMs)}</td>
            <td className="py-1.5 text-right text-ink-2">{formatDuration(month.idleMs)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function Analytics() {
  const [selection, setSelection] = useRangeSelection("month");
  const { range } = selection;
  const [focusIds, setFocusIds] = useState<number[]>([]);
  const report = useData(() => api.getReport(range, focusIds.length ? focusIds : undefined), [range, focusIds]);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const weekLabel = useCallback((key: string) => formatDate(key), []);
  const data = report.data;

  const toggleFocus = (id: number) =>
    setFocusIds((current) => (current.includes(id) ? current.filter((item) => item !== id) : [...current, id]));

  return (
    <>
      <PageHeader title="Analytics">
        <DateRangePicker value={selection} onChange={setSelection} />
      </PageHeader>

      {report.error && <ErrorNote message={report.error} />}
      {data && (
        <div className="grid gap-x-8 gap-y-7 lg:grid-cols-[3fr_2fr]">
          <Section title="Active time per day">
            <TrendLine buckets={data.daily} />
          </Section>
          <Section title="Active and idle">
            <ActiveIdleSplit totals={data.totals} />
          </Section>
          <Section title="Weekly totals">
            <ActivityBars buckets={data.weekly} label={weekLabel} height={200} ariaLabel="Active and idle time by week" />
          </Section>
          <Section title="Monthly summary">
            <MonthlyTable months={data.monthly} />
          </Section>
          <Section title="Applications">
            <RankedList items={data.applications} empty="No applications recorded in this period." limit={10} />
          </Section>
          <Section title="Projects">
            <RankedList items={data.projects} empty="No project activity in this period." limit={10} useItemColor />
          </Section>
          <Section title="Categories" className="lg:col-span-2">
            <ShareBar items={data.categories} empty="No categorized activity in this period." />
          </Section>
          <Section title="When you were active" className="lg:col-span-2">
            <p className="mb-2 text-xs text-ink-3">
              Darker hours hold more active time. Choose categories to see when you did that kind of work.
            </p>
            <div className="mb-3 flex flex-wrap gap-1.5">
              {categories.map((category) => {
                const chosen = focusIds.includes(category.id);
                return (
                  <button
                    key={category.id}
                    type="button"
                    aria-pressed={chosen}
                    onClick={() => toggleFocus(category.id)}
                    className={`inline-flex h-7 items-center gap-1.5 rounded-md border px-2 text-xs transition-colors ${
                      chosen ? "border-pen bg-pen-soft text-ink" : "border-rule-strong text-ink-2 hover:text-ink"
                    }`}
                  >
                    <ColorSwatch color={category.color} size={8} />
                    {category.name}
                  </button>
                );
              })}
            </div>
            <HourHeatmap cells={data.heatmap} />
          </Section>
        </div>
      )}
    </>
  );
}
