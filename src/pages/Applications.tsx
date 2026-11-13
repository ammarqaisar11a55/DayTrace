import { AppWindow, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { AppIcon } from "../components/AppIcon";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { CategoryOptions } from "../components/SessionDialog";
import { Badge, Button, Card, ConfirmDialog, Dialog, Empty, ErrorNote, Field, Input, PageHeader, Select } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { ApplicationUsage, Category } from "../types";
import { formatDuration } from "../utils/format";

function EditDialog({
  application,
  categories,
  onClose,
  onChanged,
}: {
  application: ApplicationUsage | null;
  categories: Category[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { attempt, notify } = useApp();
  const [displayName, setDisplayName] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!application) return;
    setDisplayName(application.displayName ?? "");
    setCategoryId(application.defaultCategoryId?.toString() ?? "");
  }, [application]);

  if (!application) return null;

  const save = async () => {
    await attempt(() =>
      api.updateApplication(application.id, {
        displayName: displayName.trim() || null,
        defaultCategoryId: categoryId ? Number(categoryId) : null,
      }),
    );
    notify("Application saved");
    onChanged();
    onClose();
  };

  const exclude = async () => {
    await attempt(() => api.addExclusion("application", application.platformIdentifier));
    notify(`${application.name} will no longer be recorded`);
    onChanged();
    onClose();
  };

  const remove = async () => {
    setConfirming(false);
    await attempt(() => api.deleteApplication(application.id));
    notify("Application and its history deleted");
    onChanged();
    onClose();
  };

  return (
    <>
      <Dialog title={application.displayName ?? application.name} open={!confirming} onClose={onClose}>
        {application.executablePath && (
          <p className="selectable mb-5 rounded-sm bg-hover/60 px-2.5 py-1.5 font-mono text-[11px] break-all text-fg-2">{application.executablePath}</p>
        )}
        <div className="space-y-3">
          <Field label="Display name">
            <Input value={displayName} placeholder={application.name} onChange={(event) => setDisplayName(event.target.value)} />
          </Field>
          <Field label="Default category" hint="Used when no rule matches.">
            <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
              <CategoryOptions categories={categories} />
            </Select>
          </Field>
        </div>
        <div className="mt-5 flex flex-wrap items-center justify-between gap-2">
          <div className="flex gap-2">
            <Button onClick={() => void exclude()}>Stop recording</Button>
            <Button tone="danger" onClick={() => setConfirming(true)}>
              Delete history
            </Button>
          </div>
          <div className="flex gap-2">
            <Button onClick={onClose}>Cancel</Button>
            <Button tone="primary" onClick={() => void save()}>
              Save changes
            </Button>
          </div>
        </div>
      </Dialog>
      <ConfirmDialog
        open={confirming}
        title={`Delete all history for ${application.name}?`}
        body={`This removes ${application.sessionCount} sessions in the selected period and every older one. This cannot be undone.`}
        action="Delete history"
        onConfirm={() => void remove()}
        onClose={() => setConfirming(false)}
      />
    </>
  );
}

type SortKey = "usage" | "name" | "category";

export function Applications() {
  const [selection, setSelection] = useRangeSelection("week");
  const [editing, setEditing] = useState<ApplicationUsage | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortKey>("usage");
  const applications = useData(() => api.listApplicationUsage(selection.range), [selection.range]);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const all = applications.data ?? [];
  const tracked = all.reduce((sum, application) => sum + application.totalMs, 0);
  const top = Math.max(1, ...all.map((application) => application.totalMs));
  const categoryOf = (id: number | null) => categories.find((category) => category.id === id);
  const nameOf = (application: ApplicationUsage) => application.displayName ?? application.name;

  const list = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = needle
      ? all.filter((application) =>
          [nameOf(application), application.name, application.executablePath ?? ""].some((text) =>
            text.toLowerCase().includes(needle),
          ),
        )
      : all;
    const byName = (a: ApplicationUsage, b: ApplicationUsage) => nameOf(a).localeCompare(nameOf(b));
    const sorted = [...matches];
    if (sort === "name") sorted.sort(byName);
    if (sort === "category")
      sorted.sort(
        (a, b) =>
          (categoryOf(a.defaultCategoryId)?.name ?? "\uffff").localeCompare(categoryOf(b.defaultCategoryId)?.name ?? "\uffff") ||
          byName(a, b),
      );
    return sorted;
  }, [all, query, sort, categories]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      <PageHeader title="Applications" description="Set a display name and default category for each application.">
        <DateRangePicker value={selection} onChange={setSelection} />
      </PageHeader>
      {applications.error && <ErrorNote message={applications.error} />}
      {applications.data && all.length === 0 && (
        <Card>
          <Empty icon={AppWindow}>Applications appear here once tracking has recorded them.</Empty>
        </Card>
      )}
      {all.length > 0 && (
        <Card className="p-2">
          <div className="flex flex-wrap items-center gap-2 p-2 pb-3">
            <div className="relative min-w-48 flex-1">
              <Search aria-hidden size={14} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-fg-3" />
              <Input
                type="search"
                aria-label="Search applications"
                placeholder="Search applications"
                className="w-full pl-8"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
              />
            </div>
            <Select aria-label="Sort by" className="w-44" value={sort} onChange={(event) => setSort(event.target.value as SortKey)}>
              <option value="usage">Most used first</option>
              <option value="name">Name</option>
              <option value="category">Category</option>
            </Select>
          </div>
          <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)_4.5rem_4rem] gap-3 border-y border-line px-3 py-2 text-[11px] font-medium tracking-wide text-fg-3 uppercase">
            <span>Application</span>
            <span>Usage</span>
            <span className="text-right">Share</span>
            <span className="text-right">Time</span>
          </div>
          {list.length === 0 && <Empty>No application matches “{query.trim()}”.</Empty>}
          <ul className="pt-1">
            {list.map((application) => {
              const name = nameOf(application);
              const category = categoryOf(application.defaultCategoryId);
              return (
                <li key={application.id}>
                  <button
                    type="button"
                    onClick={() => setEditing(application)}
                    className="grid w-full grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)_4.5rem_4rem] items-center gap-3 rounded-sm px-3 py-2 text-left transition-colors hover:bg-hover"
                  >
                    <span className="flex min-w-0 items-center gap-3">
                      <AppIcon id={application.id} name={name} size={26} />
                      <span className="min-w-0">
                        <span className="block truncate text-[13px] font-medium">{name}</span>
                        <span className="mt-0.5 flex">
                          {category ? (
                            <Badge>
                              <ColorDot color={category.color} />
                              {category.name}
                            </Badge>
                          ) : (
                            <span className="text-xs text-fg-3">No default category</span>
                          )}
                        </span>
                      </span>
                    </span>
                    <span className="h-1.5 overflow-hidden rounded-full bg-hover">
                      <span className="block h-full rounded-full bg-brand" style={{ width: `${(application.totalMs / top) * 100}%` }} />
                    </span>
                    <span className="text-right text-xs text-fg-3">
                      {tracked > 0 ? `${Math.round((application.totalMs / tracked) * 100)}%` : "0%"}
                    </span>
                    <span className="text-right text-[13px] text-fg-2">{formatDuration(application.totalMs)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </Card>
      )}
      <EditDialog
        application={editing}
        categories={categories}
        onClose={() => setEditing(null)}
        onChanged={applications.reload}
      />
    </>
  );
}

function ColorDot({ color }: { color: string }) {
  return <span aria-hidden className="size-1.5 rounded-full" style={{ background: color }} />;
}
