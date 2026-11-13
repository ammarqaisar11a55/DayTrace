import { ArrowDown, ArrowUp, Plus, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useData } from "../hooks/useData";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type {
  Category,
  Condition,
  ConditionField,
  ConditionOperator,
  DateRange,
  Rule,
  RuleInput,
  RulePreview,
} from "../types";
import { formatDateTime, formatDuration, presetRange } from "../utils/format";
import { CategoryOptions } from "./SessionDialog";
import { Button, Dialog, Empty, Field, IconButton, Input, Section, Select, Switch } from "./ui";

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
  categories,
  onClose,
  onChanged,
}: {
  open: boolean;
  rule: Rule | null;
  categories: Category[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { attempt, notify, settings } = useApp();
  const [name, setName] = useState("");
  const [conditions, setConditions] = useState<Condition[]>([]);
  const [categoryId, setCategoryId] = useState("");

  useEffect(() => {
    if (!open) return;
    setName(rule?.name ?? "");
    setConditions(rule?.conditions ?? [{ field: "application", operator: "equals", value: "" }]);
    setCategoryId(rule?.targetCategoryId.toString() ?? "");
  }, [open, rule]);

  const save = async () => {
    const input: RuleInput = {
      name,
      conditions,
      targetCategoryId: Number(categoryId),
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
        <Field label="Assign to category">
          <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
            <CategoryOptions categories={categories} empty="Choose a category" />
          </Select>
        </Field>
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <Button onClick={onClose}>Cancel</Button>
        <Button tone="primary" disabled={!categoryId} onClick={() => void save()}>
          {rule ? "Save changes" : "Create rule"}
        </Button>
      </div>
    </Dialog>
  );
}

function RulePreviewDialog({
  preview,
  range,
  categories,
  onClose,
  onApplied,
}: {
  preview: RulePreview | null;
  range: DateRange;
  categories: Category[];
  onClose: () => void;
  onApplied: () => void;
}) {
  const { attempt, notify } = useApp();
  if (!preview) return null;

  const categoryName = (id: number | null) =>
    categories.find((category) => category.id === id)?.name ?? "Uncategorized";

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
      <p className="mb-3 text-[13px] text-fg-2">
        {preview.changed} of {preview.examined} sessions would change. {preview.skippedManual} sessions you assigned by
        hand stay as they are.
      </p>
      {preview.samples.length > 0 && (
        <div className="max-h-72 overflow-y-auto border-y border-line">
          <table className="w-full text-left text-xs">
            <thead className="sticky top-0 bg-surface text-fg-3">
              <tr>
                <th className="py-1.5 font-medium">Session</th>
                <th className="py-1.5 font-medium">Now</th>
                <th className="py-1.5 font-medium">Becomes</th>
              </tr>
            </thead>
            <tbody>
              {preview.samples.map((change) => (
                <tr key={change.sessionId} className="border-t border-line">
                  <td className="py-1.5 pr-3">
                    {change.applicationName}
                    <span className="block text-fg-3">
                      {formatDateTime(change.startTime)}, {formatDuration(change.durationSeconds * 1000)}
                    </span>
                  </td>
                  <td className="py-1.5 pr-3 text-fg-2">{categoryName(change.oldCategoryId)}</td>
                  <td className="py-1.5">{categoryName(change.newCategoryId)}</td>
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

export function Rules({ categories }: { categories: Category[] }) {
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

  const targetName = (rule: Rule) => categories.find((category) => category.id === rule.targetCategoryId)?.name;

  return (
    <Section
      title="Classification rules"
      actions={
        <div className="flex gap-1.5">
          <Button
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
        <Empty>Rules assign categories automatically as activity is recorded.</Empty>
      ) : (
        <>
          <p className="mb-2 text-xs text-fg-3">Rules run from the top, and the first match sets the category.</p>
          <ol className="-mx-2">
            {list.map((rule, index) => (
              <li key={rule.id} className="flex items-center gap-2 rounded px-2 py-1 hover:bg-line/40">
                <span className="w-5 shrink-0 text-right text-xs text-fg-3">{index + 1}</span>
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left"
                  onClick={() => {
                    setEditing(rule);
                    setOpen(true);
                  }}
                >
                  <span className={`block truncate text-[13px] ${rule.enabled ? "" : "text-fg-3"}`}>{rule.name}</span>
                  <span className="block truncate text-xs text-fg-3">{targetName(rule)}</span>
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
        categories={categories}
        onClose={() => setOpen(false)}
        onChanged={rules.reload}
      />
      <RulePreviewDialog
        preview={preview}
        range={previewRange}
        categories={categories}
        onClose={() => setPreview(null)}
        onApplied={rules.reload}
      />
    </Section>
  );
}
