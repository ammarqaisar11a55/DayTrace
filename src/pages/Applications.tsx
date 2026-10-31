import { useEffect, useState } from "react";
import { AppIcon } from "../components/AppIcon";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { CategoryOptions } from "../components/SessionDialog";
import { Button, ConfirmDialog, Dialog, Empty, ErrorNote, Field, Input, PageHeader, Select } from "../components/ui";
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
          <p className="selectable mb-4 text-xs break-all text-ink-3">{application.executablePath}</p>
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

export function Applications() {
  const [selection, setSelection] = useRangeSelection("week");
  const [editing, setEditing] = useState<ApplicationUsage | null>(null);
  const applications = useData(() => api.listApplicationUsage(selection.range), [selection.range]);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const list = applications.data ?? [];
  const top = list[0]?.totalMs || 1;
  const categoryName = (id: number | null) => categories.find((category) => category.id === id)?.name;

  return (
    <>
      <PageHeader title="Applications">
        <DateRangePicker value={selection} onChange={setSelection} />
      </PageHeader>
      {applications.error && <ErrorNote message={applications.error} />}
      {applications.data && list.length === 0 && (
        <Empty>Applications appear here once tracking has recorded them.</Empty>
      )}
      <ul className="-mx-2">
        {list.map((application) => {
          const name = application.displayName ?? application.name;
          return (
            <li key={application.id}>
              <button
                type="button"
                onClick={() => setEditing(application)}
                className="flex w-full items-center gap-3 rounded px-2 py-2 text-left hover:bg-rule/40"
              >
                <AppIcon id={application.id} name={name} size={24} />
                <span className="w-56 min-w-0 shrink-0">
                  <span className="block truncate text-[13px]">{name}</span>
                  <span className="block truncate text-xs text-ink-3">{categoryName(application.defaultCategoryId) ?? "No default category"}</span>
                </span>
                <span className="h-1 min-w-0 flex-1 bg-rule/50">
                  <span
                    className="block h-full rounded-r-[2px] bg-pen"
                    style={{ width: `${(application.totalMs / top) * 100}%` }}
                  />
                </span>
                <span className="w-16 shrink-0 text-right text-[13px] text-ink-2">
                  {formatDuration(application.totalMs)}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      <EditDialog
        application={editing}
        categories={categories}
        onClose={() => setEditing(null)}
        onChanged={applications.reload}
      />
    </>
  );
}
