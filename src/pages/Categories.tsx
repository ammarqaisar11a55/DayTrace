import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { ProjectOptions } from "../components/SessionDialog";
import { Button, ColorSwatch, ConfirmDialog, Dialog, ErrorNote, Field, Input, PageHeader, Select } from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Category, Project } from "../types";
import { formatDuration } from "../utils/format";

function CategoryDialog({
  open,
  category,
  projects,
  onClose,
  onChanged,
}: {
  open: boolean;
  category: Category | null;
  projects: Project[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { attempt, notify } = useApp();
  const [name, setName] = useState("");
  const [color, setColor] = useState("#2a78d6");
  const [projectId, setProjectId] = useState("");
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName(category?.name ?? "");
    setColor(category?.color ?? "#2a78d6");
    setProjectId(category?.defaultProjectId?.toString() ?? "");
  }, [open, category]);

  const save = async () => {
    const input = { name, color, defaultProjectId: projectId ? Number(projectId) : null };
    const saved = await attempt(() =>
      category ? api.updateCategory(category.id, input) : api.createCategory(input),
    );
    if (saved) {
      notify(category ? "Category saved" : "Category created");
      onChanged();
      onClose();
    }
  };

  const remove = async () => {
    if (!category) return;
    setConfirming(false);
    await attempt(() => api.deleteCategory(category.id));
    notify("Category deleted");
    onChanged();
    onClose();
  };

  return (
    <>
      <Dialog title={category ? "Edit category" : "New category"} open={open && !confirming} onClose={onClose}>
        <div className="space-y-3">
          <div className="grid grid-cols-[1fr_auto] gap-3">
            <Field label="Name">
              <Input value={name} autoFocus onChange={(event) => setName(event.target.value)} />
            </Field>
            <Field label="Color">
              <input
                type="color"
                value={color}
                onChange={(event) => setColor(event.target.value)}
                className="h-8 w-12 cursor-pointer rounded-md border border-rule-strong bg-sheet p-0.5"
              />
            </Field>
          </div>
          <Field label="Project for this category" hint="Sessions in this category go here when nothing else sets a project.">
            <Select value={projectId} onChange={(event) => setProjectId(event.target.value)}>
              <ProjectOptions projects={projects} />
            </Select>
          </Field>
        </div>
        <div className="mt-5 flex items-center justify-between">
          {category ? (
            <Button tone="danger" onClick={() => setConfirming(true)}>
              Delete
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button onClick={onClose}>Cancel</Button>
            <Button tone="primary" onClick={() => void save()}>
              {category ? "Save changes" : "Create category"}
            </Button>
          </div>
        </div>
      </Dialog>
      <ConfirmDialog
        open={confirming}
        title={`Delete ${category?.name}?`}
        body="Sessions keep their time and become uncategorized. Rules that only set this category are deleted too."
        action="Delete category"
        onConfirm={() => void remove()}
        onClose={() => setConfirming(false)}
      />
    </>
  );
}

export function Categories() {
  const [selection, setSelection] = useRangeSelection("week");
  const [editing, setEditing] = useState<Category | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const categories = useData(() => api.listCategories(), []);
  const report = useData(() => api.getReport(selection.range), [selection.range]);
  const { data: projects = [] } = useData(() => api.listProjects(false), []);

  const usage = new Map((report.data?.categories ?? []).map((item) => [item.id, item]));

  return (
    <>
      <PageHeader title="Categories">
        <DateRangePicker value={selection} onChange={setSelection} />
        <Button
          tone="primary"
          onClick={() => {
            setEditing(null);
            setDialogOpen(true);
          }}
        >
          <Plus size={14} />
          New category
        </Button>
      </PageHeader>
      {categories.error && <ErrorNote message={categories.error} />}
      <ul className="-mx-2">
        {(categories.data ?? []).map((category) => {
          const item = usage.get(category.id);
          return (
            <li key={category.id}>
              <button
                type="button"
                onClick={() => {
                  setEditing(category);
                  setDialogOpen(true);
                }}
                className="flex w-full items-center gap-3 rounded px-2 py-2 text-left hover:bg-rule/40"
              >
                <ColorSwatch color={category.color} size={14} />
                <span className="min-w-0 flex-1 truncate text-[13px]">{category.name}</span>
                <span className="text-xs text-ink-3">
                  {projects.find((project) => project.id === category.defaultProjectId)?.name}
                </span>
                <span className="w-12 text-right text-xs text-ink-3">
                  {item ? `${Math.round(item.share * 100)}%` : ""}
                </span>
                <span className="w-16 text-right text-[13px] text-ink-2">{formatDuration(item?.ms ?? 0)}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <CategoryDialog
        open={dialogOpen}
        category={editing}
        projects={projects}
        onClose={() => setDialogOpen(false)}
        onChanged={categories.reload}
      />
    </>
  );
}
