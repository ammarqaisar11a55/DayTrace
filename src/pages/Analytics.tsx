import { useCallback, useState } from "react";
import { ActiveIdleSplit, ActivityBars, HourHeatmap, RankedList, ShareBar, TrendLine } from "../components/charts";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { ColorSwatch, ErrorNote, PageHeader, Section } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import type { Bucket } from "../types";
import { formatDate, formatDuration, parseDateKey } from "../utils/format";

function MonthlyTable({ months }: { months: Bucket[] }) {
  return (
    <table className="w-full text-left text-[13px]">
      <thead className="text-[11px] tracking-wide text-fg-3 uppercase">
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
          <tr key={month.key} className="border-t border-line transition-colors hover:bg-hover/50">
            <td className="py-1.5">
              {parseDateKey(`${month.key}-01`).toLocaleDateString(undefined, { month: "long", year: "numeric" })}
            </td>
            <td className="py-1.5 text-right">{formatDuration(month.activeMs)}</td>
            <td className="py-1.5 text-right text-fg-2">{formatDuration(month.idleMs)}</td>
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
        <div className="grid gap-5 lg:grid-cols-[3fr_2fr]">
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
          <Section title="Categories">
            <ShareBar items={data.categories} empty="No categorized activity in this period." />
          </Section>
          <Section
            title="When you were active"
            description="Stronger color means more active time. Choose categories to see when you did that kind of work."
            className="lg:col-span-2"
          >
            <div className="mb-4 flex flex-wrap gap-1.5">
              {categories.map((category) => {
                const chosen = focusIds.includes(category.id);
                return (
                  <button
                    key={category.id}
                    type="button"
                    aria-pressed={chosen}
                    onClick={() => toggleFocus(category.id)}
                    className={`inline-flex h-7 items-center gap-1.5 rounded-full border px-2.5 text-xs transition-colors duration-150 ${
                      chosen ? "border-accent/60 bg-primary-soft text-fg" : "border-line text-fg-2 hover:bg-hover hover:text-fg"
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
