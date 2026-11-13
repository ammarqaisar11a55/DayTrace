import { useMemo, useState } from "react";
import type { DateRange, Session } from "../types";
import { daysInRange, formatClock, formatDate, formatDuration, isAway, parseDateKey, sessionTitle, toDateKey } from "../utils/format";
import { ColorSwatch } from "./ui";

/** Segments narrower than this are joined, so a month stays a few hundred marks. */
const MIN_SEGMENT_FRACTION = 1 / 480;

interface Segment {
  start: number;
  end: number;
  color: string | null;
  away: boolean;
  session: Session;
}

function dayBounds(key: string): [number, number] {
  const start = parseDateKey(key);
  const end = parseDateKey(key);
  end.setDate(end.getDate() + 1);
  return [start.getTime(), end.getTime()];
}

function segmentColor(session: Session): string {
  if (session.sessionType === "unknown") return "var(--dt-idle)";
  return session.categoryColor ?? "var(--dt-primary)";
}

function buildSegments(sessions: Session[], dayStart: number, dayEnd: number): Segment[] {
  const minimum = (dayEnd - dayStart) * MIN_SEGMENT_FRACTION;
  const segments: Segment[] = [];

  for (const session of sessions) {
    const start = Math.max(session.startTime, dayStart);
    const end = Math.min(session.endTime, dayEnd);
    if (end <= start) continue;

    const away = isAway(session);
    const color = away ? null : segmentColor(session);
    const last = segments.at(-1);
    const touching = last && start - last.end < minimum;
    if (last && touching && last.away === away && (last.color === color || end - start < minimum)) {
      last.end = end;
    } else {
      segments.push({ start, end, color, away, session });
    }
  }
  return segments;
}

function Ribbon({
  label,
  dayKey,
  sessions,
  height,
  onSelect,
  onHover,
}: {
  label?: string;
  dayKey: string;
  sessions: Session[];
  height: number;
  onSelect?: (session: Session) => void;
  onHover: (hover: { session: Session; x: number } | null) => void;
}) {
  const [dayStart, dayEnd] = dayBounds(dayKey);
  const span = dayEnd - dayStart;
  const segments = useMemo(() => buildSegments(sessions, dayStart, dayEnd), [sessions, dayStart, dayEnd]);
  const now = Date.now();
  const nowPercent = now >= dayStart && now < dayEnd ? ((now - dayStart) / span) * 100 : null;

  return (
    <div className="flex items-center gap-3">
      {label && <span className="w-14 shrink-0 text-right text-xs text-fg-3">{label}</span>}
      <div className="relative min-w-0 flex-1 bg-line/40" style={{ height }}>
        {segments.map((segment) => {
          const left = ((segment.start - dayStart) / span) * 100;
          const width = ((segment.end - segment.start) / span) * 100;
          return (
            <button
              key={segment.start}
              type="button"
              tabIndex={onSelect ? 0 : -1}
              aria-label={`${sessionTitle(segment.session)}, ${formatClock(segment.start)} to ${formatClock(segment.end)}`}
              onClick={() => onSelect?.(segment.session)}
              onMouseEnter={(event) => onHover({ session: segment.session, x: event.clientX })}
              onMouseMove={(event) => onHover({ session: segment.session, x: event.clientX })}
              onMouseLeave={() => onHover(null)}
              className={`absolute inset-y-0 min-w-px ${segment.away ? "hatch-idle" : ""}`}
              style={{
                left: `${left}%`,
                width: `${width}%`,
                background: segment.away ? undefined : (segment.color ?? undefined),
              }}
            />
          );
        })}
        {nowPercent !== null && (
          <span
            aria-hidden
            className="pointer-events-none absolute -inset-y-1 w-0.5 bg-fg"
            style={{ left: `${nowPercent}%` }}
          />
        )}
      </div>
    </div>
  );
}

function HourAxis({ indent }: { indent: boolean }) {
  const hours = [0, 3, 6, 9, 12, 15, 18, 21, 24];
  return (
    <div className="mt-1.5 flex gap-3">
      {indent && <span className="w-14 shrink-0" />}
      <div className="relative h-4 min-w-0 flex-1 text-[11px] text-fg-3">
        {hours.map((hour) => (
          <span
            key={hour}
            className="absolute -translate-x-1/2 first:translate-x-0 last:-translate-x-full"
            style={{ left: `${(hour / 24) * 100}%` }}
          >
            {String(hour).padStart(2, "0")}
          </span>
        ))}
      </div>
    </div>
  );
}

function Legend({ sessions }: { sessions: Session[] }) {
  const items = useMemo(() => {
    const categories = new Map<string, string>();
    let uncategorized = false;
    let unknown = false;
    let away = false;
    for (const session of sessions) {
      if (isAway(session)) away = true;
      else if (session.sessionType === "unknown") unknown = true;
      else if (session.categoryName && session.categoryColor) categories.set(session.categoryName, session.categoryColor);
      else uncategorized = true;
    }
    return { categories: [...categories], uncategorized, unknown, away };
  }, [sessions]);

  return (
    <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-xs text-fg-2">
      {items.categories.map(([name, color]) => (
        <span key={name} className="inline-flex items-center gap-1.5">
          <ColorSwatch color={color} />
          {name}
        </span>
      ))}
      {items.uncategorized && (
        <span className="inline-flex items-center gap-1.5">
          <ColorSwatch color="var(--dt-primary)" />
          Uncategorized
        </span>
      )}
      {items.unknown && (
        <span className="inline-flex items-center gap-1.5">
          <ColorSwatch color="var(--dt-idle)" />
          Unidentified
        </span>
      )}
      {items.away && (
        <span className="inline-flex items-center gap-1.5">
          <span aria-hidden className="hatch-idle inline-block size-2.5 rounded-[3px]" />
          Idle or locked
        </span>
      )}
      <span className="inline-flex items-center gap-1.5">
        <span aria-hidden className="inline-block size-2.5 rounded-[3px] bg-line/70" />
        Not recorded
      </span>
    </div>
  );
}

/**
 * The record of each day, drawn on a 24-hour scale. One day is a tall ribbon.
 * Longer ranges stack one line per day, so the shape of a week or month shows
 * when you were at the computer as well as how much.
 */
export function DayRibbon({
  range,
  sessions,
  onSelect,
}: {
  range: DateRange;
  sessions: Session[];
  onSelect?: (session: Session) => void;
}) {
  const [hover, setHover] = useState<{ session: Session; x: number } | null>(null);
  const days = useMemo(() => daysInRange(range).reverse(), [range]);
  const single = days.length === 1;
  const height = single ? 48 : days.length <= 7 ? 16 : 7;

  const byDay = useMemo(() => {
    const groups = new Map<string, Session[]>();
    for (const session of sessions) {
      const first = toDateKey(new Date(session.startTime));
      const last = toDateKey(new Date(session.endTime));
      for (const key of first === last ? [first] : [first, last]) {
        const group = groups.get(key);
        if (group) group.push(session);
        else groups.set(key, [session]);
      }
    }
    return groups;
  }, [sessions]);

  return (
    <div className="relative" onMouseLeave={() => setHover(null)}>
      <div className={days.length > 7 ? "space-y-px" : "space-y-1"}>
        {days.map((day) => (
          <Ribbon
            key={day}
            dayKey={day}
            label={single ? undefined : formatDate(day, { weekday: "short", day: "numeric" })}
            sessions={byDay.get(day) ?? []}
            height={height}
            onSelect={onSelect}
            onHover={setHover}
          />
        ))}
      </div>
      <HourAxis indent={!single} />
      <Legend sessions={sessions} />
      {hover && (
        <div
          role="status"
          className="pointer-events-none fixed top-auto z-20 mt-1 max-w-64 -translate-x-1/2 rounded-md border border-line-strong bg-surface px-2.5 py-1.5 text-xs"
          style={{ left: hover.x }}
        >
          <div className="font-medium">{sessionTitle(hover.session)}</div>
          <div className="text-fg-2">
            {formatClock(hover.session.startTime)} to {formatClock(hover.session.endTime)},{" "}
            {formatDuration(hover.session.endTime - hover.session.startTime)}
          </div>
          {hover.session.categoryName && <div className="text-fg-3">{hover.session.categoryName}</div>}
        </div>
      )}
    </div>
  );
}
