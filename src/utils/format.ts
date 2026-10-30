import type { DateRange, Session } from "../types";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

export function formatDuration(ms: number): string {
  if (ms < MINUTE) {
    const seconds = Math.floor(ms / 1000);
    return seconds > 0 ? `${seconds}s` : "0m";
  }
  const totalMinutes = Math.floor(ms / MINUTE);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) return `${minutes}m`;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

export function formatHours(ms: number): string {
  return `${(ms / HOUR).toFixed(1)}h`;
}

export function formatClock(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function formatDate(key: string, options?: Intl.DateTimeFormatOptions): string {
  return parseDateKey(key).toLocaleDateString(undefined, options ?? { month: "short", day: "numeric" });
}

export function formatDateTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function toDateKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

export function parseDateKey(key: string): Date {
  const [year = 1970, month = 1, day = 1] = key.split("-").map(Number);
  return new Date(year, month - 1, day);
}

export function addDays(key: string, days: number): string {
  const date = parseDateKey(key);
  date.setDate(date.getDate() + days);
  return toDateKey(date);
}

export type Preset = "today" | "yesterday" | "week" | "month";

export const PRESETS: { id: Preset; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "yesterday", label: "Yesterday" },
  { id: "week", label: "Last 7 days" },
  { id: "month", label: "Last 30 days" },
];

export function presetRange(preset: Preset, today = toDateKey(new Date())): DateRange {
  switch (preset) {
    case "today":
      return { from: today, to: today };
    case "yesterday": {
      const yesterday = addDays(today, -1);
      return { from: yesterday, to: yesterday };
    }
    case "week":
      return { from: addDays(today, -6), to: today };
    case "month":
      return { from: addDays(today, -29), to: today };
  }
}

export function daysInRange(range: DateRange): string[] {
  const days: string[] = [];
  for (let day = range.from; day <= range.to; day = addDays(day, 1)) days.push(day);
  return days;
}

export function toLocalInput(ms: number): string {
  const date = new Date(ms);
  const time = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
  return `${toDateKey(date)}T${time}`;
}

export function fromLocalInput(value: string): number {
  return new Date(value).getTime();
}

export function sessionTitle(session: Session): string {
  switch (session.sessionType) {
    case "idle":
      return "Idle";
    case "locked":
      return "Locked";
    case "unknown":
      return "Unidentified activity";
    case "manual":
      return session.label ?? "Manual entry";
    case "active":
      return session.applicationName ?? "Removed application";
  }
}

export function isAway(session: Session): boolean {
  return session.sessionType === "idle" || session.sessionType === "locked";
}

export interface SessionGroup {
  key: string;
  title: string;
  startTime: number;
  endTime: number;
  totalMs: number;
  sessions: Session[];
}

function groupKey(session: Session): string {
  return [session.sessionType, session.applicationId, session.projectId, session.categoryId, session.label].join("|");
}

/** Joins neighbouring sessions of the same activity. Expects chronological order. */
export function groupConsecutive(sessions: Session[], maxGapMs = MINUTE): SessionGroup[] {
  const groups: SessionGroup[] = [];
  for (const session of sessions) {
    const last = groups.at(-1);
    const ms = session.endTime - session.startTime;
    if (last && last.key === groupKey(session) && session.startTime - last.endTime <= maxGapMs) {
      last.endTime = session.endTime;
      last.totalMs += ms;
      last.sessions.push(session);
    } else {
      groups.push({
        key: groupKey(session),
        title: sessionTitle(session),
        startTime: session.startTime,
        endTime: session.endTime,
        totalMs: ms,
        sessions: [session],
      });
    }
  }
  return groups;
}
