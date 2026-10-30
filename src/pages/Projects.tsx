import { Archive, ArchiveRestore, ArrowDown, ArrowUp, Plus, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { RankedList } from "../components/charts";
import { DateRangePicker, useRangeSelection } from "../components/DateRangePicker";
import { CategoryOptions, ProjectOptions } from "../components/SessionDialog";
import {
  Button,
  ColorSwatch,
  ConfirmDialog,
  Dialog,
  Empty,
  ErrorNote,
  Field,
  IconButton,
  Input,
  PageHeader,
  Section,
  Select,
  Switch,
} from "../components/ui";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type {
  Category,
  Condition,
  ConditionField,
  ConditionOperator,
  DateRange,
  Project,
  Rule,
  RuleInput,
  RulePreview,
} from "../types";
import { formatDateTime, formatDuration, presetRange } from "../utils/format";

const COLORS = ["#2a78d6", "#1baf7a", "#6250d6", "#eb6834", "#e87ba4", "#008300", "#eda100", "#e34948"];

const FIELDS: { value: ConditionField; label: string }[] = [
  { value: "application", label: "Application" },
  { value: "executablePath", label: "Program path" },
  { value: "windowTitle", label: "Window title" },
];

const OPERATORS: { value: ConditionOperator; label: string }[] = [
  { value: "equals", label: "is" },
  { value: "contains", label: "contains" },
  { value: "startsWith", label: "starts with" },
  { value: "endsWith", label: "ends with" },
];

function ProjectDialog({
  open,
  project,
  projects,
  onClose,
  onChanged,
}: {
  open: boolean;
  project: Project | null;
  projects: Project[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { attempt, notify } = useApp();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [color, setColor] = useState(COLORS[0]!);
  const [parentId, setParentId] = useState("");
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName(project?.name ?? "");
    setDescription(project?.description ?? "");
    setColor(project?.color ?? COLORS[projects.length % COLORS.length]!);
    setParentId(project?.parentId?.toString() ?? "");
  }, [open, project, projects.length]);

  const parents = projects.filter((candidate) => candidate.id !== project?.id);

  const save = async () => {
    const input = { name, description, color, parentId: parentId ? Number(parentId) : null };
    const saved = await attempt(() => (project ? api.updateProject(project.id, input) : api.createProject(input)));
    if (saved) {
      notify(project ? "Project saved" : "Project created");
      onChanged();
      onClose();
    }
  };

  const remove = async () => {
    if (!project) return;
    setConfirming(false);
    await attempt(() => api.deleteProject(project.id));
    notify("Project deleted");
    onChanged();
    onClose();
  };

  return (
    <>
      <Dialog title={project ? "Edit project" : "New project"} open={open && !confirming} onClose={onClose}>
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
          <Field label="Inside project">
            <Select value={parentId} onChange={(event) => setParentId(event.target.value)}>
              <option value="">None</option>
              {parents.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {candidate.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Description">
            <Input value={description} onChange={(event) => setDescription(event.target.value)} />
          </Field>
        </div>
        <div className="mt-5 flex items-center justify-between">
          {project ? (
            <Button tone="danger" onClick={() => setConfirming(true)}>
              <Trash2 size={14} />
              Delete
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button onClick={onClose}>Cancel</Button>
            <Button tone="primary" onClick={() => void save()}>
              {project ? "Save changes" : "Create project"}
            </Button>
          </div>
        </div>
      </Dialog>
      <ConfirmDialog
        open={confirming}
        title={`Delete ${project?.name}?`}
        body="Its sessions keep their time and become unassigned. Archive the project instead to keep its history together."
        action="Delete project"
        onConfirm={() => void remove()}
        onClose={() => setConfirming(false)}
      />
    </>
  );
}

function ProjectDetail({
  project,
  range,
  usageMs,
  onEdit,
  onChanged,
}: {
  project: Project;
  range: DateRange;
  usageMs: number;
  onEdit: () => void;
  onChanged: () => void;
}) {
  const { attempt } = useApp();
  const breakdown = useData(() => api.projectBreakdown(project.id, range), [project.id, range]);
  const archived = project.status === "archived";

  return (
    <div>
      <div className="mb-3 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <ColorSwatch color={project.color} size={12} />
            <h2 className="truncate text-[15px] font-semibold">{project.name}</h2>
          </div>
          {project.description && <p className="mt-0.5 text-[13px] text-ink-2">{project.description}</p>}
          <p className="mt-1 text-[22px] leading-tight font-semibold tracking-tight">{formatDuration(usageMs)}</p>
        </div>
        <div className="flex shrink-0 gap-1">
          <Button onClick={onEdit}>Edit</Button>
          <IconButton
            label={archived ? "Restore project" : "Archive project"}
            onClick={() =>
              void attempt(async () => {
                await api.archiveProject(project.id, !archived);
                onChanged();
              })
            }
          >
            {archived ? <ArchiveRestore size={15} /> : <Archive size={15} />}
          </IconButton>
        </div>
      </div>
      <RankedList items={breakdown.data ?? []} empty="Nothing recorded for this project in this period." />
    </div>
  );
}

function ConditionRow({
  condition,
  onChange,
  onRemove,
  canRemove,
}: {
  condition: Condition;
  onChange: (condition: Condition) => void;
  onRemove: () => void;
  canRemove: boolean;
}) {
  return (
    <div className="flex items-center gap-2">
      <Select
        aria-label="Field"
        className="w-36 shrink-0"
        value={condition.field}
        onChange={(event) => onChange({ ...condition, field: event.target.value as ConditionField })}
      >
        {FIELDS.map((field) => (
          <option key={field.value} value={field.value}>
            {field.label}
          </option>
        ))}
      </Select>
      <Select
        aria-label="Comparison"
        className="w-32 shrink-0"
        value={condition.operator}
        onChange={(event) => onChange({ ...condition, operator: event.target.value as ConditionOperator })}
      >
        {OPERATORS.map((operator) => (
          <option key={operator.value} value={operator.value}>
            {operator.label}
          </option>
        ))}
      </Select>
      <Input
        aria-label="Value"
        value={condition.value}
        onChange={(event) => onChange({ ...condition, value: event.target.value })}
      />
      <IconButton label="Remove condition" disabled={!canRemove} onClick={onRemove}>
        <X size={14} />
      </IconButton>
    </div>
  );
}

function RuleDialog({
  open,
  rule,
  projects,
  categories,
  onClose,
  onChanged,
}: {
  open: boolean;
  rule: Rule | null;
  projects: Project[];
  categories: Category[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { attempt, notify, settings } = useApp();
  const [name, setName] = useState("");
  const [conditions, setConditions] = useState<Condition[]>([]);
  const [projectId, setProjectId] = useState("");
  const [categoryId, setCategoryId] = useState("");

  useEffect(() => {
    if (!open) return;
    setName(rule?.name ?? "");
    setConditions(rule?.conditions ?? [{ field: "application", operator: "equals", value: "" }]);
    setProjectId(rule?.targetProjectId?.toString() ?? "");
    setCategoryId(rule?.targetCategoryId?.toString() ?? "");
  }, [open, rule]);

  const save = async () => {
    const input: RuleInput = {
      name,
      conditions,
      targetProjectId: projectId ? Number(projectId) : null,
      targetCategoryId: categoryId ? Number(categoryId) : null,
      enabled: rule?.enabled ?? true,
    };
    const saved = await attempt(() => (rule ? api.updateRule(rule.id, input) : api.createRule(input)));
    if (saved) {
      notify(rule ? "Rule saved" : "Rule created");
      onChanged();
      onClose();
    }
  };

  const usesTitle = conditions.some((condition) => condition.field === "windowTitle");

  return (
    <Dialog title={rule ? "Edit rule" : "New rule"} open={open} onClose={onClose} wide>
      <div className="space-y-3">
        <Field label="Name">
          <Input value={name} autoFocus onChange={(event) => setName(event.target.value)} placeholder="Thesis writing" />
        </Field>
        <div>
          <span className="mb-1 block text-[13px] font-medium">When all of these match</span>
          <div className="space-y-2">
            {conditions.map((condition, index) => (
              <ConditionRow
                key={index}
                condition={condition}
                canRemove={conditions.length > 1}
                onChange={(next) => setConditions(conditions.map((item, i) => (i === index ? next : item)))}
                onRemove={() => setConditions(conditions.filter((_, i) => i !== index))}
              />
            ))}
          </div>
          <Button
            tone="quiet"
            className="mt-1"
            onClick={() => setConditions([...conditions, { field: "application", operator: "contains", value: "" }])}
          >
            <Plus size={14} />
            Add condition
          </Button>
          {usesTitle && !settings.collectWindowTitles && (
            <p className="mt-1 text-xs text-warn">
              Window titles are not being collected, so this condition will not match. You can turn them on in Settings.
            </p>
          )}
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Assign to project">
            <Select value={projectId} onChange={(event) => setProjectId(event.target.value)}>
              <ProjectOptions projects={projects} />
            </Select>
          </Field>
          <Field label="Assign to category">
            <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
              <CategoryOptions categories={categories} />
            </Select>
          </Field>
        </div>
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <Button onClick={onClose}>Cancel</Button>
        <Button tone="primary" onClick={() => void save()}>
          {rule ? "Save changes" : "Create rule"}
        </Button>
      </div>
    </Dialog>
  );
}

function RulePreviewDialog({
  preview,
  range,
  projects,
  categories,
  onClose,
  onApplied,
}: {
  preview: RulePreview | null;
  range: DateRange;
  projects: Project[];
  categories: Category[];
  onClose: () => void;
  onApplied: () => void;
}) {
  const { attempt, notify } = useApp();
  if (!preview) return null;

  const describe = (projectId: number | null, categoryId: number | null) =>
    [
      projects.find((project) => project.id === projectId)?.name,
      categories.find((category) => category.id === categoryId)?.name,
    ]
      .filter(Boolean)
      .join(", ") || "Unassigned";

  const apply = async () => {
    const applied = await attempt(() => api.applyRules(range));
    if (applied) {
      notify(`${applied.changed} sessions reclassified`);
      onApplied();
      onClose();
    }
  };

  return (
    <Dialog title="Preview rule changes" open onClose={onClose} wide>
      <p className="mb-3 text-[13px] text-ink-2">
        {preview.changed} of {preview.examined} sessions would change. {preview.skippedManual} sessions you assigned by
        hand stay as they are.
      </p>
      {preview.samples.length > 0 && (
        <div className="max-h-72 overflow-y-auto border-y border-rule">
          <table className="w-full text-left text-xs">
            <thead className="sticky top-0 bg-sheet text-ink-3">
              <tr>
                <th className="py-1.5 font-medium">Session</th>
                <th className="py-1.5 font-medium">Now</th>
                <th className="py-1.5 font-medium">Becomes</th>
              </tr>
            </thead>
            <tbody>
              {preview.samples.map((change) => (
                <tr key={change.sessionId} className="border-t border-rule">
                  <td className="py-1.5 pr-3">
                    {change.applicationName}
                    <span className="block text-ink-3">
                      {formatDateTime(change.startTime)}, {formatDuration(change.durationSeconds * 1000)}
                    </span>
                  </td>
                  <td className="py-1.5 pr-3 text-ink-2">{describe(change.oldProjectId, change.oldCategoryId)}</td>
                  <td className="py-1.5">{describe(change.newProjectId, change.newCategoryId)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="mt-5 flex justify-end gap-2">
        <Button onClick={onClose}>Cancel</Button>
        <Button tone="primary" disabled={preview.changed === 0} onClick={() => void apply()}>
          Apply to {preview.changed} sessions
        </Button>
      </div>
    </Dialog>
  );
}

function Rules({ projects, categories }: { projects: Project[]; categories: Category[] }) {
  const { attempt } = useApp();
  const rules = useData(() => api.listRules(), []);
  const [editing, setEditing] = useState<Rule | null>(null);
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<RulePreview | null>(null);
  const list = rules.data ?? [];
  const previewRange = useMemo(() => presetRange("month"), []);

  const move = (index: number, offset: number) => {
    const ids = list.map((rule) => rule.id);
    const [id] = ids.splice(index, 1);
    ids.splice(index + offset, 0, id!);
    void attempt(async () => {
      await api.reorderRules(ids);
      rules.reload();
    });
  };

  const toggle = (rule: Rule, enabled: boolean) =>
    attempt(async () => {
      const { id: _id, priority: _priority, ...input } = rule;
      await api.updateRule(rule.id, { ...input, enabled });
      rules.reload();
    });

  const targetName = (rule: Rule) =>
    [
      projects.find((project) => project.id === rule.targetProjectId)?.name,
      categories.find((category) => category.id === rule.targetCategoryId)?.name,
    ]
      .filter(Boolean)
      .join(", ");

  return (
    <Section
      title="Classification rules"
      actions={
        <div className="flex gap-1.5">
          <Button
            disabled={list.length === 0}
            onClick={() =>
              void attempt(async () => {
                setPreview(await api.previewRules(previewRange));
              })
            }
          >
            Preview last 30 days
          </Button>
          <Button
            onClick={() => {
              setEditing(null);
              setOpen(true);
            }}
          >
            <Plus size={14} />
            New rule
          </Button>
        </div>
      }
    >
      {list.length === 0 ? (
        <Empty>Rules assign projects and categories automatically as activity is recorded.</Empty>
      ) : (
        <>
          <p className="mb-2 text-xs text-ink-3">
            Rules run from the top. The first match sets the project, and the first match sets the category.
          </p>
          <ol className="-mx-2">
            {list.map((rule, index) => (
              <li key={rule.id} className="flex items-center gap-2 rounded px-2 py-1 hover:bg-rule/40">
                <span className="w-5 shrink-0 text-right text-xs text-ink-3">{index + 1}</span>
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left"
                  onClick={() => {
                    setEditing(rule);
                    setOpen(true);
                  }}
                >
                  <span className={`block truncate text-[13px] ${rule.enabled ? "" : "text-ink-3"}`}>{rule.name}</span>
                  <span className="block truncate text-xs text-ink-3">{targetName(rule)}</span>
                </button>
                <IconButton label="Move up" disabled={index === 0} onClick={() => move(index, -1)}>
                  <ArrowUp size={14} />
                </IconButton>
                <IconButton label="Move down" disabled={index === list.length - 1} onClick={() => move(index, 1)}>
                  <ArrowDown size={14} />
                </IconButton>
                <Switch checked={rule.enabled} label={`Enable ${rule.name}`} onChange={(value) => void toggle(rule, value)} />
                <IconButton
                  label="Delete rule"
                  onClick={() =>
                    void attempt(async () => {
                      await api.deleteRule(rule.id);
                      rules.reload();
                    })
                  }
                >
                  <Trash2 size={14} />
                </IconButton>
              </li>
            ))}
          </ol>
        </>
      )}
      <RuleDialog
        open={open}
        rule={editing}
        projects={projects}
        categories={categories}
        onClose={() => setOpen(false)}
        onChanged={rules.reload}
      />
      <RulePreviewDialog
        preview={preview}
        range={previewRange}
        projects={projects}
        categories={categories}
        onClose={() => setPreview(null)}
        onApplied={rules.reload}
      />
    </Section>
  );
}

export function Projects() {
  const [selection, setSelection] = useRangeSelection("month");
  const [showArchived, setShowArchived] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<Project | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  const projects = useData(() => api.listProjects(true), []);
  const report = useData(() => api.getReport(selection.range), [selection.range]);
  const { data: categories = [] } = useData(() => api.listCategories(), []);

  const all = projects.data ?? [];
  const visible = all.filter((project) => showArchived || project.status === "active");
  const active = all.filter((project) => project.status === "active");
  const selected = all.find((project) => project.id === selectedId) ?? visible[0] ?? null;
  const usage = (id: number) => report.data?.projects.find((item) => item.id === id)?.ms ?? 0;

  const tree = useMemo(() => {
    const ordered: { project: Project; depth: number }[] = [];
    const visit = (parentId: number | null, depth: number) => {
      for (const project of visible.filter((item) => item.parentId === parentId)) {
        ordered.push({ project, depth });
        visit(project.id, depth + 1);
      }
    };
    visit(null, 0);
    for (const project of visible) {
      if (!ordered.some((item) => item.project.id === project.id)) ordered.push({ project, depth: 0 });
    }
    return ordered;
  }, [visible]);

  return (
    <>
      <PageHeader title="Projects">
        <DateRangePicker value={selection} onChange={setSelection} />
        <Button
          tone="primary"
          onClick={() => {
            setEditing(null);
            setDialogOpen(true);
          }}
        >
          <Plus size={14} />
          New project
        </Button>
      </PageHeader>

      {projects.error && <ErrorNote message={projects.error} />}
      {projects.data && all.length === 0 ? (
        <Empty>Create a project to see how your time divides between the things you work on.</Empty>
      ) : (
        <div className="mb-8 grid gap-x-8 gap-y-6 lg:grid-cols-[2fr_3fr]">
          <div>
            <ul className="-mx-2">
              {tree.map(({ project, depth }) => (
                <li key={project.id}>
                  <button
                    type="button"
                    aria-current={selected?.id === project.id}
                    onClick={() => setSelectedId(project.id)}
                    className={`flex w-full items-center gap-2 rounded py-1.5 pr-2 text-left text-[13px] ${
                      selected?.id === project.id ? "bg-pen-soft" : "hover:bg-rule/40"
                    }`}
                    style={{ paddingLeft: 8 + depth * 16 }}
                  >
                    <ColorSwatch color={project.color} />
                    <span className={`min-w-0 flex-1 truncate ${project.status === "archived" ? "text-ink-3" : ""}`}>
                      {project.name}
                    </span>
                    <span className="text-ink-2">{formatDuration(usage(project.id))}</span>
                  </button>
                </li>
              ))}
            </ul>
            {all.some((project) => project.status === "archived") && (
              <label className="mt-2 flex items-center gap-2 text-xs text-ink-3">
                <Switch checked={showArchived} onChange={setShowArchived} label="Show archived projects" />
                Show archived projects
              </label>
            )}
          </div>
          {selected && (
            <ProjectDetail
              project={selected}
              range={selection.range}
              usageMs={usage(selected.id)}
              onEdit={() => {
                setEditing(selected);
                setDialogOpen(true);
              }}
              onChanged={projects.reload}
            />
          )}
        </div>
      )}

      <Rules projects={active} categories={categories} />

      <ProjectDialog
        open={dialogOpen}
        project={editing}
        projects={active}
        onClose={() => setDialogOpen(false)}
        onChanged={projects.reload}
      />
    </>
  );
}
