import { useId } from "react";

const SOLID = [
  [102, 129, 105],
  [348, 129, 62],
  [170, 226, 123],
  [102, 323, 148],
  [335, 323, 75],
];

const SOFT = [
  [225, 129, 105],
  [102, 226, 50],
  [311, 226, 99],
  [268, 323, 50],
];

/** The DayTrace mark, drawn from the same geometry as assets/icon.svg. */
export function Logo({ size = 20 }: { size?: number }) {
  const gradient = `logo${useId().replace(/[^\w-]/g, "")}`;
  return (
    <svg aria-hidden width={size} height={size} viewBox="31 31 450 450" className="shrink-0">
      <defs>
        <linearGradient id={gradient} x1="31" y1="31" x2="481" y2="481" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#6200B8" />
          <stop offset="0.48" stopColor="#8B20E8" />
          <stop offset="1" stopColor="#BD65FF" />
        </linearGradient>
      </defs>
      <rect x="31" y="31" width="450" height="450" rx="96" fill={`url(#${gradient})`} />
      {SOLID.map(([x, y, width]) => (
        <rect key={`${x}-${y}`} x={x} y={y} width={width} height="60" rx="14" fill="#FFFFFF" />
      ))}
      {SOFT.map(([x, y, width]) => (
        <rect key={`${x}-${y}`} x={x} y={y} width={width} height="60" rx="14" fill="#D7B4FF" fillOpacity="0.72" />
      ))}
    </svg>
  );
}
