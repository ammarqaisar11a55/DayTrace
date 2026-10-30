import { CircleAlert, CircleCheck, CircleSlash } from "lucide-react";
import { useState } from "react";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Capability, Support } from "../types";
import { Button } from "./ui";

const SUPPORT: Record<Support, { icon: typeof CircleCheck; label: string; className: string }> = {
  available: { icon: CircleCheck, label: "Available", className: "text-pen" },
  limited: { icon: CircleAlert, label: "Limited", className: "text-warn" },
  unavailable: { icon: CircleSlash, label: "Not available", className: "text-danger" },
};

function CapabilityRow({ name, capability }: { name: string; capability: Capability }) {
  const { icon: Icon, label, className } = SUPPORT[capability.support];
  return (
    <li className="flex gap-2.5 py-1.5">
      <Icon size={15} className={`mt-0.5 shrink-0 ${className}`} aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="flex justify-between gap-3 text-[13px]">
          <span>{name}</span>
          <span className="shrink-0 text-ink-3">{label}</span>
        </div>
        {capability.detail && <p className="text-xs text-ink-3">{capability.detail}</p>}
      </div>
    </li>
  );
}

function ShellIntegration() {
  const { status, attempt, notify } = useApp();
  const [busy, setBusy] = useState(false);
  const state = status?.capabilities.integration;
  if (!state) return null;

  const change = async (action: () => Promise<void>, message: string) => {
    setBusy(true);
    const done = await attempt(async () => {
      await action();
      return true;
    });
    setBusy(false);
    if (done) notify(message);
  };

  return (
    <div className="mt-3 rounded-md border border-rule-strong p-3">
      <h3 className="text-[13px] font-medium">GNOME Shell integration</h3>
      {state === "notInstalled" && (
        <>
          <p className="mt-1 mb-2.5 text-xs text-ink-2">
            GNOME keeps the focused window private from applications. This small extension, included with DayTrace, tells
            DayTrace which application has focus. Without it, DayTrace records only that you were active.
          </p>
          <Button
            tone="primary"
            disabled={busy}
            onClick={() => void change(api.installShellIntegration, "Installed. Log out and back in to activate it.")}
          >
            Install integration
          </Button>
        </>
      )}
      {state === "pendingRestart" && (
        <>
          <p className="mt-1 mb-2.5 text-xs text-ink-2">
            Installed. GNOME loads new extensions at login, so log out and back in to activate it.
          </p>
          <Button disabled={busy} onClick={() => void change(api.removeShellIntegration, "Integration removed")}>
            Remove integration
          </Button>
        </>
      )}
      {state === "active" && (
        <>
          <p className="mt-1 mb-2.5 text-xs text-ink-2">Active. DayTrace can see which application has focus.</p>
          <Button disabled={busy} onClick={() => void change(api.removeShellIntegration, "Integration removed")}>
            Remove integration
          </Button>
        </>
      )}
    </div>
  );
}

/** What this computer can actually report, stated plainly. */
export function CapabilityList() {
  const { status } = useApp();
  const capabilities = status?.capabilities;
  if (!capabilities) return <p className="text-ink-3">Checking this computer...</p>;

  return (
    <div>
      <p className="mb-1 text-xs text-ink-3">
        {capabilities.desktop} on {capabilities.displayServer}
      </p>
      <ul className="divide-y divide-rule">
        <CapabilityRow name="Focused application" capability={capabilities.foregroundApp} />
        <CapabilityRow name="Program path" capability={capabilities.executablePath} />
        <CapabilityRow name="Window titles" capability={capabilities.windowTitle} />
        <CapabilityRow name="Idle detection" capability={capabilities.idle} />
        <CapabilityRow name="Screen lock" capability={capabilities.lock} />
        <CapabilityRow name="Sleep and wake" capability={capabilities.suspend} />
      </ul>
      <ShellIntegration />
    </div>
  );
}
