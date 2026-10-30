import { Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../services/api";
import { useApp } from "../stores/app";
import type { Category, Project, Session } from "../types";
import { formatDuration, fromLocalInput, sessionTitle, toLocalInput } from "../utils/format";
import { Button, ConfirmDialog, Dialog, Field, Input, Select } from "./ui";

export function ProjectOptions({ projects }: { projects: Project[] }) {
  return (
    <>
      <option value="">No project</option>
      {projects.map((project) => (
        <option key={project.id} value={project.id}>
          {project.name}
        </option>
      ))}
    </>
  );
}

export function CategoryOptions({ categories }: { categories: Category[] }) {
  return (
    <>
      <option value="">No category</option>
      {categories.map((category) => (
        <option key={category.id} value={category.id}>
          {category.name}
        </option>
      ))}
    </>
  );
}

const numberOrNull = (value: string) => (value === "" ? null : Number(value));

/** Inspect and correct one recorded session. */
export function SessionDialog({
  session,
  projects,
  categories,
  onClose,
  onChanged,
}: {
  session: Session | null;
  projects: Project[];
  categories: Category[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const { attempt, notify } = useApp();
  const [projectId, setProjectId] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [label, setLabel] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!session) return;
    setProjectId(session.projectId?.toString() ?? "");
    setCategoryId(session.categoryId?.toString() ?? "");
    setLabel(session.label ?? "");
    setStart(toLocalInput(session.startTime));
    setEnd(toLocalInput(session.endTime));
  }, [session]);

  if (!session) return null;

  const timesChanged = start !== toLocalInput(session.startTime) || end !== toLocalInput(session.endTime);

  const save = async () => {
    const saved = await attempt(() =>
      api.updateSession(session.id, {
        projectId: numberOrNull(projectId),
        categoryId: numberOrNull(categoryId),
        label: label || null,
        ...(timesChanged ? { startTime: fromLocalInput(start), endTime: fromLocalInput(end) } : {}),
      }),
    );
    if (saved) {
      notify("Session saved");
      onChanged();
      onClose();
    }
  };

  const remove = async () => {
    setConfirming(false);
    await attempt(() => api.deleteSession(session.id));
    notify("Session deleted");
    onChanged();
    onClose();
  };

  return (
    <>
      <Dialog title={sessionTitle(session)} open={!confirming} onClose={onClose}>
        <p className="mb-4 text-[13px] text-ink-2">
          {formatDuration(session.endTime - session.startTime)}
          {session.isOpen ? ", still in progress" : ""}
        </p>
        {(session.executablePath || session.windowTitle) && (
          <dl className="selectable mb-4 space-y-1 text-xs text-ink-2">
            {session.executablePath && (
              <div>
                <dt className="inline text-ink-3">Program: </dt>
                <dd className="inline break-all">{session.executablePath}</dd>
              </div>
            )}
            {session.windowTitle && (
              <div>
                <dt className="inline text-ink-3">Window: </dt>
                <dd className="inline break-all">{session.windowTitle}</dd>
              </div>
            )}
          </dl>
        )}
        <div className="grid grid-cols-2 gap-3">
          <Field label="Project">
            <Select value={projectId} onChange={(event) => setProjectId(event.target.value)}>
              <ProjectOptions projects={projects} />
            </Select>
          </Field>
          <Field label="Category">
            <Select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
              <CategoryOptions categories={categories} />
            </Select>
          </Field>
          <div className="col-span-2">
            <Field label="Label">
              <Input value={label} onChange={(event) => setLabel(event.target.value)} placeholder="What was this?" />
            </Field>
          </div>
          <Field label="Started">
            <Input type="datetime-local" value={start} disabled={session.isOpen} onChange={(event) => setStart(event.target.value)} />
          </Field>
          <Field label="Ended">
            <Input type="datetime-local" value={end} disabled={session.isOpen} onChange={(event) => setEnd(event.target.value)} />
          </Field>
        </div>
        <div className="mt-5 flex items-center justify-between">
          <Button tone="danger" disabled={session.isOpen} onClick={() => setConfirming(true)}>
            <Trash2 size={14} />
            Delete
          </Button>
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
        title="Delete this session?"
        body="The time will show as not recorded. This cannot be undone."
        action="Delete session"
        onConfirm={() => void remove()}
        onClose={() => setConfirming(false)}
      />
    </>
  );
}
