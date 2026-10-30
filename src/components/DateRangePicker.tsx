import { useState } from "react";
import type { DateRange } from "../types";
import { PRESETS, presetRange, toDateKey, type Preset } from "../utils/format";
import { Input } from "./ui";

export interface RangeSelection {
  preset: Preset | "custom";
  range: DateRange;
}

export function useRangeSelection(initial: Preset = "today") {
  return useState<RangeSelection>({ preset: initial, range: presetRange(initial) });
}

export function DateRangePicker({
  value,
  onChange,
}: {
  value: RangeSelection;
  onChange: (selection: RangeSelection) => void;
}) {
  const today = toDateKey(new Date());
  return (
    <div className="flex flex-wrap items-center gap-2">
      <div role="group" aria-label="Date range" className="flex rounded-md border border-rule-strong bg-sheet p-0.5">
        {PRESETS.map((preset) => (
          <button
            key={preset.id}
            type="button"
            aria-pressed={value.preset === preset.id}
            onClick={() => onChange({ preset: preset.id, range: presetRange(preset.id) })}
            className={`h-7 rounded px-2.5 text-[13px] transition-colors ${
              value.preset === preset.id ? "bg-pen font-medium text-on-pen" : "text-ink-2 hover:text-ink"
            }`}
          >
            {preset.label}
          </button>
        ))}
        <button
          type="button"
          aria-pressed={value.preset === "custom"}
          onClick={() => onChange({ preset: "custom", range: value.range })}
          className={`h-7 rounded px-2.5 text-[13px] transition-colors ${
            value.preset === "custom" ? "bg-pen font-medium text-on-pen" : "text-ink-2 hover:text-ink"
          }`}
        >
          Custom
        </button>
      </div>
      {value.preset === "custom" && (
        <div className="flex items-center gap-1.5">
          <Input
            type="date"
            aria-label="First day"
            className="w-36"
            value={value.range.from}
            max={value.range.to}
            onChange={(event) =>
              event.target.value &&
              onChange({ preset: "custom", range: { ...value.range, from: event.target.value } })
            }
          />
          <span className="text-ink-3">to</span>
          <Input
            type="date"
            aria-label="Last day"
            className="w-36"
            value={value.range.to}
            min={value.range.from}
            max={today}
            onChange={(event) =>
              event.target.value &&
              onChange({ preset: "custom", range: { ...value.range, to: event.target.value } })
            }
          />
        </div>
      )}
    </div>
  );
}
