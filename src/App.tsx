import { useEffect, useState } from "react";
import { Shell, type PageId } from "./layouts/Shell";
import { Analytics } from "./pages/Analytics";
import { Applications } from "./pages/Applications";
import { Categories } from "./pages/Categories";
import { Dashboard } from "./pages/Dashboard";
import { Onboarding } from "./pages/Onboarding";
import { SettingsPage } from "./pages/Settings";
import { Timeline } from "./pages/Timeline";
import { AppProvider, useApp } from "./stores/app";

const PAGE_IDS: PageId[] = ["dashboard", "timeline", "applications", "categories", "analytics", "settings"];

function pageFromHash(): PageId {
  const hash = window.location.hash.slice(1) as PageId;
  return PAGE_IDS.includes(hash) ? hash : "dashboard";
}

function Pages() {
  const { settings } = useApp();
  const [page, setPage] = useState<PageId>(pageFromHash);

  useEffect(() => {
    const onChange = () => setPage(pageFromHash());
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);

  if (!settings.onboardingCompleted) return <Onboarding />;

  return (
    <Shell page={page}>
      {page === "dashboard" && <Dashboard />}
      {page === "timeline" && <Timeline />}
      {page === "applications" && <Applications />}
      {page === "categories" && <Categories />}
      {page === "analytics" && <Analytics />}
      {page === "settings" && <SettingsPage />}
    </Shell>
  );
}

export function App() {
  return (
    <AppProvider fallback={<div className="h-full bg-paper" />}>
      <Pages />
    </AppProvider>
  );
}
