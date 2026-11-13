import type { TrackerStatus } from "../types";
import { formatClock } from "./format";

/** What the tracker is doing, in words for the sidebar and dashboard. */
export function describe(status: TrackerStatus | null): { title: string; detail?: string; live: boolean } {
  switch (status?.state) {
    case "tracking":
      return {
        title: "Tracking",
        detail: status.currentApplication ?? "Application not identified",
        live: true,
      };
    case "idle":
      return { title: "Idle", detail: "Waiting for activity", live: false };
    case "locked":
      return { title: "Locked", live: false };
    case "paused":
      return {
        title: "Paused",
        detail: status.pausedUntil ? `Until ${formatClock(status.pausedUntil)}` : "Until you resume",
        live: false,
      };
    case "manual":
      return { title: "Timer running", detail: status.manualLabel ?? "Manual entry", live: true };
    default:
      return { title: "Tracking is off", detail: "Turn it on in Settings", live: false };
  }
}
