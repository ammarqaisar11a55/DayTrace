import { describe, expect, it } from "vitest";
import type { Session } from "../types";
import {
  addDays,
  daysInRange,
  formatDuration,
  groupConsecutive,
  presetRange,
  sessionTitle,
} from "./format";

function session(overrides: Partial<Session>): Session {
  return {
    id: 1,
    applicationId: 1,
    applicationName: "Code",
    executablePath: null,
    categoryId: null,
    categoryName: null,
    categoryColor: null,
    startTime: 0,
    endTime: 60_000,
    durationSeconds: 60,
    sessionType: "active",
    windowTitle: null,
    label: null,
    manualOverride: false,
    isOpen: false,
    ...overrides,
  };
}

describe("formatDuration", () => {
  it("rounds down to whole units", () => {
    expect(formatDuration(0)).toBe("0m");
    expect(formatDuration(45_000)).toBe("45s");
    expect(formatDuration(60_000)).toBe("1m");
    expect(formatDuration(3_600_000)).toBe("1h");
    expect(formatDuration(12_000_000)).toBe("3h 20m");
  });
});

describe("dates", () => {
  it("crosses month and year boundaries", () => {
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
  });

  it("builds inclusive preset ranges", () => {
    expect(presetRange("yesterday", "2026-05-02")).toEqual({ from: "2026-05-01", to: "2026-05-01" });
    expect(daysInRange(presetRange("week", "2026-05-10"))).toHaveLength(7);
    expect(daysInRange(presetRange("month", "2026-05-30"))).toHaveLength(30);
  });

  it("is not thrown off by daylight saving days", () => {
    expect(daysInRange({ from: "2026-03-07", to: "2026-03-09" })).toHaveLength(3);
    expect(daysInRange({ from: "2026-10-31", to: "2026-11-02" })).toHaveLength(3);
  });
});

describe("groupConsecutive", () => {
  it("joins neighbouring sessions of one activity", () => {
    const groups = groupConsecutive([
      session({ id: 1, startTime: 0, endTime: 60_000 }),
      session({ id: 2, startTime: 90_000, endTime: 120_000 }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0]?.sessions).toHaveLength(2);
    expect(groups[0]?.totalMs).toBe(90_000);
  });

  it("keeps sessions apart when an application, category or long gap separates them", () => {
    expect(
      groupConsecutive([
        session({ id: 1 }),
        session({ id: 2, applicationId: 2, startTime: 60_000, endTime: 90_000 }),
        session({ id: 3, startTime: 90_000, endTime: 120_000 }),
      ]),
    ).toHaveLength(3);
    expect(
      groupConsecutive([session({ id: 1 }), session({ id: 2, startTime: 600_000, endTime: 660_000 })]),
    ).toHaveLength(2);
    expect(
      groupConsecutive([session({ id: 1 }), session({ id: 2, categoryId: 5, startTime: 60_000 })]),
    ).toHaveLength(2);
  });
});

describe("sessionTitle", () => {
  it("never names an application for unidentified activity", () => {
    expect(sessionTitle(session({ sessionType: "unknown", applicationName: null }))).toBe(
      "Unidentified activity",
    );
    expect(sessionTitle(session({ sessionType: "idle" }))).toBe("Idle");
    expect(sessionTitle(session({ sessionType: "manual", label: "Reading" }))).toBe("Reading");
  });
});
