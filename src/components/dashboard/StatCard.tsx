import type { LucideIcon } from "lucide-react";
import { Card } from "../ui";

export function StatCard({
  icon: Icon,
  label,
  value,
  meta,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  meta?: string;
}) {
  return (
    <Card className="p-4">
      <div className="mb-3 flex items-center gap-2.5">
        <span className="inline-flex size-8 items-center justify-center rounded-sm bg-primary-soft text-accent">
          <Icon size={16} strokeWidth={1.75} />
        </span>
        <span className="text-[13px] text-fg-2">{label}</span>
      </div>
      <p className="text-[26px] leading-tight font-semibold tracking-tight">{value}</p>
      {meta && <p className="mt-1 truncate text-xs text-fg-3">{meta}</p>}
    </Card>
  );
}
