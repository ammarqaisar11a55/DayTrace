import { useMemo } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Bucket, HourCell, RankedItem } from "../types";
import { formatDate, formatDuration, formatHours } from "../utils/format";
import { ColorSwatch, Empty } from "./ui";

const HOUR = 3_600_000;
const MINUTE = 60_000;
const AXIS = { fontSize: 11, fill: "var(--dt-text-muted)" };

interface Point {
  label: string;
  active: number;
  idle: number;
}

function ChartTooltip({
  active,
  payload,
  label,
  unit = HOUR,
}: {
  active?: boolean;
  payload?: readonly { name?: unknown; value?: unknown; color?: string }[];
  label?: string | number;
  unit?: number;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-md border border-line-strong bg-surface px-2.5 py-1.5 text-xs">
      <div className="mb-0.5 font-medium">{label}</div>
      {payload.map((entry) => (
        <div key={String(entry.name)} className="flex items-center gap-1.5 text-fg-2">
          <ColorSwatch color={entry.color ?? null} size={8} />
          {String(entry.name)}: {formatDuration(Number(entry.value) * unit)}
        </div>
      ))}
    </div>
  );
}

function SeriesLegend() {
  return (
    <div className="mb-2 flex gap-4 text-xs text-fg-2">
      <span className="inline-flex items-center gap-1.5">
        <ColorSwatch color="var(--dt-primary)" />
        Active
      </span>
      <span className="inline-flex items-center gap-1.5">
        <ColorSwatch color="var(--dt-idle)" />
        Idle
      </span>
    </div>
  );
}

function toPoints(buckets: Bucket[], label: (key: string) => string, unit = HOUR): Point[] {
  return buckets.map((bucket) => ({
    label: label(bucket.key),
    active: bucket.activeMs / unit,
    idle: bucket.idleMs / unit,
  }));
}

/** Active and idle time per bucket, stacked on one hours axis. */
export function ActivityBars({
  buckets,
  label,
  height = 220,
  ariaLabel,
  perHour,
}: {
  buckets: Bucket[];
  label: (key: string) => string;
  height?: number;
  ariaLabel: string;
  /** Buckets are single clock hours, so the axis is fixed at 60 minutes. */
  perHour?: boolean;
}) {
  const unit = perHour ? MINUTE : HOUR;
  const points = useMemo(() => toPoints(buckets, label, unit), [buckets, label, unit]);
  return (
    <figure aria-label={ariaLabel}>
      <SeriesLegend />
      <ResponsiveContainer width="100%" height={height}>
        <BarChart data={points} margin={{ top: 4, right: 0, bottom: 0, left: -24 }} barCategoryGap="28%">
          <CartesianGrid vertical={false} stroke="var(--dt-border)" />
          <XAxis dataKey="label" tick={AXIS} tickLine={false} axisLine={{ stroke: "var(--dt-border-strong)" }} interval="preserveStartEnd" minTickGap={18} />
          <YAxis
            tick={AXIS}
            tickLine={false}
            axisLine={false}
            tickFormatter={(value: number) => (perHour ? `${value}m` : `${value}h`)}
            allowDecimals={false}
            {...(perHour ? { domain: [0, 60], ticks: [0, 15, 30, 45, 60] } : {})}
          />
          <Tooltip content={<ChartTooltip unit={unit} />} cursor={{ fill: "var(--dt-border)", opacity: 0.4 }} />
          <Bar dataKey="active" name="Active" stackId="time" fill="var(--dt-primary)" isAnimationActive={false} />
          <Bar dataKey="idle" name="Idle" stackId="time" fill="var(--dt-idle)" radius={[3, 3, 0, 0]} isAnimationActive={false} />
        </BarChart>
      </ResponsiveContainer>
    </figure>
  );
}

/** Active time over a range, for reading its direction rather than exact days. */
export function TrendLine({ buckets, height = 220 }: { buckets: Bucket[]; height?: number }) {
  const points = useMemo(() => toPoints(buckets, (key) => formatDate(key)), [buckets]);
  return (
    <figure aria-label="Active time per day">
      <ResponsiveContainer width="100%" height={height}>
        <AreaChart data={points} margin={{ top: 4, right: 8, bottom: 0, left: -24 }}>
          <CartesianGrid vertical={false} stroke="var(--dt-border)" />
          <XAxis dataKey="label" tick={AXIS} tickLine={false} axisLine={{ stroke: "var(--dt-border-strong)" }} interval="preserveStartEnd" minTickGap={28} />
          <YAxis tick={AXIS} tickLine={false} axisLine={false} tickFormatter={(value: number) => `${value}h`} allowDecimals={false} />
          <Tooltip content={<ChartTooltip />} cursor={{ stroke: "var(--dt-border-strong)" }} />
          <Area type="linear" dataKey="active" name="Active" stroke="var(--dt-primary)" strokeWidth={2} fill="var(--dt-primary-soft)" fillOpacity={0.7} isAnimationActive={false} activeDot={{ r: 4 }} />
        </AreaChart>
      </ResponsiveContainer>
    </figure>
  );
}

/** Names with a bar behind their share. Length carries the value, so one color is enough. */
export function RankedList({
  items,
  empty,
  limit = 6,
  useItemColor,
}: {
  items: RankedItem[];
  empty: string;
  limit?: number;
  useItemColor?: boolean;
}) {
  if (items.length === 0) return <Empty>{empty}</Empty>;
  const top = items[0]?.ms ?? 1;
  const shown = items.slice(0, limit);
  const rest = items.slice(limit).reduce((sum, item) => sum + item.ms, 0);

  return (
    <ul className="space-y-2.5">
      {shown.map((item) => (
        <li key={`${item.id}-${item.name}`}>
          <div className="mb-1 flex items-baseline justify-between gap-3 text-[13px]">
            <span className="truncate">{item.name}</span>
            <span className="shrink-0 text-fg-2">{formatDuration(item.ms)}</span>
          </div>
          <div className="h-1 bg-line/50">
            <div
              className="h-full rounded-r-[2px]"
              style={{
                width: `${Math.max((item.ms / top) * 100, 1)}%`,
                background: useItemColor && item.color ? item.color : "var(--dt-primary)",
              }}
            />
          </div>
        </li>
      ))}
      {rest > 0 && (
        <li className="text-xs text-fg-3">
          {items.length - limit} more, {formatDuration(rest)}
        </li>
      )}
    </ul>
  );
}

/** Parts of a whole as one stacked bar, with a legend so identity is not color alone. */
export function ShareBar({ items, empty }: { items: RankedItem[]; empty: string }) {
  if (items.length === 0) return <Empty>{empty}</Empty>;
  return (
    <div>
      <div className="flex h-3 gap-0.5" role="img" aria-label="Share of active time by category">
        {items.map((item) => (
          <div
            key={`${item.id}-${item.name}`}
            title={`${item.name}: ${formatDuration(item.ms)}`}
            className="first:rounded-l-[3px] last:rounded-r-[3px]"
            style={{ flexGrow: item.ms, flexBasis: 0, minWidth: 2, background: item.color ?? "var(--dt-primary)" }}
          />
        ))}
      </div>
      <ul className="mt-3 space-y-1.5">
        {items.map((item) => (
          <li key={`${item.id}-${item.name}`} className="flex items-center gap-2 text-[13px]">
            <ColorSwatch color={item.color ?? "var(--dt-primary)"} />
            <span className="min-w-0 flex-1 truncate">{item.name}</span>
            <span className="text-fg-3">{Math.round(item.share * 100)}%</span>
            <span className="w-16 text-right text-fg-2">{formatDuration(item.ms)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/** Time by weekday and hour on one sequential hue. Empty hours stay blank. */
export function HourHeatmap({ cells }: { cells: HourCell[] }) {
  const lookup = useMemo(() => new Map(cells.map((cell) => [`${cell.weekday}-${cell.hour}`, cell.ms])), [cells]);
  const peak = useMemo(() => Math.max(1, ...cells.map((cell) => cell.ms)), [cells]);

  if (cells.length === 0) return <Empty>No matching activity in this period.</Empty>;

  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[560px] border-separate border-spacing-0.5 text-[11px] text-fg-3">
        <caption className="sr-only">Active time by weekday and hour</caption>
        <thead>
          <tr>
            <th scope="col" className="w-9" />
            {Array.from({ length: 24 }, (_, hour) => (
              <th key={hour} scope="col" className="font-normal">
                {hour % 3 === 0 ? String(hour).padStart(2, "0") : ""}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {WEEKDAYS.map((weekday, day) => (
            <tr key={weekday}>
              <th scope="row" className="pr-1 text-right font-normal">
                {weekday}
              </th>
              {Array.from({ length: 24 }, (_, hour) => {
                const ms = lookup.get(`${day}-${hour}`) ?? 0;
                return (
                  <td
                    key={hour}
                    title={`${weekday} ${String(hour).padStart(2, "0")}:00, ${formatHours(ms)}`}
                    className="h-5 rounded-[2px]"
                    style={{
                      background:
                        ms > 0
                          ? `color-mix(in oklab, var(--dt-primary) ${Math.round(18 + (ms / peak) * 82)}%, transparent)`
                          : "color-mix(in oklab, var(--dt-border) 45%, transparent)",
                    }}
                  />
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
