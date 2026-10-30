import { useState, type ReactNode } from "react";
import { CapabilityList } from "../components/CapabilityList";
import { Button } from "../components/ui";
import { api } from "../services/api";
import { useApp } from "../stores/app";

function Bullets({ items }: { items: string[] }) {
  return (
    <ul className="space-y-1.5">
      {items.map((item) => (
        <li key={item} className="flex gap-2 text-ink-2">
          <span aria-hidden className="mt-[9px] size-1 shrink-0 rounded-full bg-pen" />
          {item}
        </li>
      ))}
    </ul>
  );
}

const STEPS: { title: string; body: ReactNode }[] = [
  {
    title: "DayTrace records how you use your computer",
    body: (
      <p className="text-ink-2">
        It notes which application has focus and for how long, and when you step away. There are no timers to start and
        nothing to remember.
      </p>
    ),
  },
  {
    title: "See where your time went",
    body: (
      <p className="text-ink-2">
        You get an honest record of each day: which applications you used, how your time divided between projects, and
        how your habits change. DayTrace shows the numbers and leaves the judgement to you.
      </p>
    ),
  },
  {
    title: "What is recorded",
    body: (
      <Bullets
        items={[
          "The name and program path of the focused application",
          "When each session starts and ends",
          "When you are idle or the screen is locked",
          "Projects, categories and labels you assign",
        ]}
      />
    ),
  },
  {
    title: "What is never recorded",
    body: (
      <>
        <Bullets
          items={[
            "Keystrokes, passwords or clipboard contents",
            "Screenshots, camera or microphone",
            "Private messages",
            "Browser history",
          ]}
        />
        <p className="mt-3 text-ink-2">Window titles can contain private details, so they stay off unless you turn them on.</p>
      </>
    ),
  },
  {
    title: "Your data stays on this computer",
    body: (
      <p className="text-ink-2">
        Everything is kept in a local database. There is no account, no cloud service and no telemetry. You can export or
        delete your history at any time from Settings. The only network request DayTrace makes is its update check, which
        you can turn off.
      </p>
    ),
  },
  {
    title: "What works on this computer",
    body: <CapabilityList />,
  },
];

export function Onboarding() {
  const { replaceSettings, attempt } = useApp();
  const [step, setStep] = useState(0);
  const current = STEPS[step]!;
  const last = step === STEPS.length - 1;

  const finish = async (enableTracking: boolean) => {
    const settings = await attempt(() => api.completeOnboarding(enableTracking));
    if (settings) replaceSettings(settings);
  };

  return (
    <main className="flex h-full items-center justify-center overflow-y-auto p-6">
      <div className="w-full max-w-lg">
        <div className="mb-6 flex gap-1" role="progressbar" aria-valuemin={1} aria-valuemax={STEPS.length} aria-valuenow={step + 1} aria-label="Setup progress">
          {STEPS.map((item, index) => (
            <span key={item.title} className={`h-1 flex-1 rounded-sm ${index <= step ? "bg-pen" : "bg-rule"}`} />
          ))}
        </div>
        <h1 className="mb-3 text-[22px] leading-tight font-semibold tracking-tight">{current.title}</h1>
        <div className="min-h-44">{current.body}</div>

        {last ? (
          <div className="mt-6">
            <p className="mb-3 text-ink-2">
              Automatic tracking stays off until you turn it on. You can change this later in Settings, and pause it at
              any time.
            </p>
            <div className="flex flex-wrap justify-between gap-2">
              <Button onClick={() => setStep(step - 1)}>Back</Button>
              <div className="flex gap-2">
                <Button onClick={() => void finish(false)}>Not now</Button>
                <Button tone="primary" onClick={() => void finish(true)}>
                  Turn on automatic tracking
                </Button>
              </div>
            </div>
          </div>
        ) : (
          <div className="mt-6 flex justify-between">
            {step > 0 ? <Button onClick={() => setStep(step - 1)}>Back</Button> : <span />}
            <Button tone="primary" onClick={() => setStep(step + 1)}>
              Continue
            </Button>
          </div>
        )}
      </div>
    </main>
  );
}
