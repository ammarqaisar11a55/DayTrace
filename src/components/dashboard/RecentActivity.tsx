import { History } from "lucide-react";
import type { Session } from "../../types";
import { formatClock, formatDuration, isAway, sessionTitle } from "../../utils/format";
import { AppIcon } from "../AppIcon";
import { ColorSwatch, Empty } from "../ui";

function SessionMark({ session }: { session: Session }) {
  if (session.sessionType === "active") return <AppIcon id={session.applicationId} name={sessionTitle(session)} size={22} />;
  return (
    <span
      aria-hidden
      className={`size-[22px] shrink-0 rounded-sm ${isAway(session) ? "hatch-idle bg-hover" : "bg-primary-soft"}`}
    />
  );
}

/** The latest sessions, newest first. Selecting one opens it for editing. */
export function RecentActivity({ sessions, onSelect }: { sessions: Session[]; onSelect: (session: Session) => void }) {
  if (sessions.length === 0) return <Empty icon={History}>Sessions appear here as they are recorded.</Empty>;
  return (
    <ul className="-mx-2">
      {sessions.map((session) => (
        <li key={session.id}>
          <button
            type="button"
            onClick={() => onSelect(session)}
            className="flex w-full items-center gap-3 rounded-sm px-2 py-2 text-left text-[13px] transition-colors hover:bg-hover"
          >
            <SessionMark session={session} />
            <span className="min-w-0 flex-1">
              <span className={`block truncate ${isAway(session) ? "text-fg-3" : ""}`}>{sessionTitle(session)}</span>
              {(session.categoryName || (session.label && session.sessionType !== "manual")) && (
                <span className="flex items-center gap-1.5 truncate text-xs text-fg-3">
                  {session.categoryName && <ColorSwatch color={session.categoryColor} size={7} />}
                  {[session.sessionType === "manual" ? null : session.label, session.categoryName].filter(Boolean).join(", ")}
                </span>
              )}
            </span>
            <span className="hidden shrink-0 text-xs whitespace-nowrap text-fg-3 sm:block">
              {formatClock(session.startTime)} to {formatClock(session.endTime)}
            </span>
            <span className="w-16 shrink-0 text-right text-fg-2">{formatDuration(session.endTime - session.startTime)}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
