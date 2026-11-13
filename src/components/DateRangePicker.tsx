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
      <div role="group" aria-label="Date range" className="flex h-9 items-center gap-0.5 rounded-md border border-line bg-surface p-1">
        {PRESETS.map((preset) => (
          <button
            key={preset.id}
            type="button"
            aria-pressed={value.preset === preset.id}
            onClick={() => onChange({ preset: preset.id, range: presetRange(preset.id) })}
            className={`h-full rounded-sm px-3 text-[13px] transition-colors duration-150 ${
              value.preset === preset.id ? "bg-primary-soft font-medium text-accent" : "text-fg-2 hover:bg-hover hover:text-fg"
            }`}
          >
            {preset.label}
          </button>
        ))}
        <button
          type="button"
          aria-pressed={value.preset === "custom"}
          onClick={() => onChange({ preset: "custom", range: value.range })}
          className={`h-full rounded-sm px-3 text-[13px] transition-colors duration-150 ${
            value.preset === "custom" ? "bg-primary-soft font-medium text-accent" : "text-fg-2 hover:bg-hover hover:text-fg"
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
          <span className="text-[13px] text-fg-3">to</span>
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
