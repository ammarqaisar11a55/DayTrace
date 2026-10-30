import { X } from "lucide-react";
import {
  useEffect,
  useRef,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";

type ButtonTone = "primary" | "secondary" | "quiet" | "danger";

const BUTTON_TONES: Record<ButtonTone, string> = {
  primary: "bg-pen text-on-pen hover:opacity-90",
  secondary: "border border-rule-strong bg-sheet text-ink hover:bg-paper",
  quiet: "text-ink-2 hover:bg-rule/60 hover:text-ink",
  danger: "border border-danger text-danger hover:bg-danger-soft",
};

export function Button({
  tone = "secondary",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { tone?: ButtonTone }) {
  return (
    <button
      type="button"
      {...props}
      className={`inline-flex h-8 items-center justify-center gap-1.5 rounded-md px-3 text-[13px] font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${BUTTON_TONES[tone]} ${className}`}
    />
  );
}

export function IconButton({
  label,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      {...props}
      className={`inline-flex size-8 items-center justify-center rounded-md text-ink-2 transition-colors hover:bg-rule/60 hover:text-ink disabled:opacity-40 ${className}`}
    />
  );
}

const FIELD_CLASS =
  "h-8 rounded-md border border-rule-strong bg-sheet px-2.5 text-[13px] text-ink placeholder:text-ink-3 disabled:opacity-50";

/** Fields fill their container unless the caller gives them a width. */
export function Input({ className = "w-full", ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`${FIELD_CLASS} ${className}`} />;
}

export function Select({ className = "w-full", ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={`${FIELD_CLASS} ${className}`} />;
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-[13px] font-medium text-ink">{label}</span>
      {children}
      {hint && <span className="mt-1 block text-xs text-ink-3">{hint}</span>}
    </label>
  );
}

export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-50 ${
        checked ? "bg-pen" : "bg-rule-strong"
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 size-4 rounded-full bg-white transition-transform ${
          checked ? "translate-x-4" : ""
        }`}
      />
    </button>
  );
}

export function Dialog({
  title,
  open,
  onClose,
  children,
  wide,
}: {
  title: string;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  const reference = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = reference.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={reference}
      onClose={() => {
        if (open) onClose();
      }}
      className={`w-[calc(100vw-48px)] rounded-lg border border-rule-strong bg-sheet p-0 ${
        wide ? "max-w-2xl" : "max-w-md"
      }`}
    >
      {open && (
        <div className="p-5">
          <div className="mb-4 flex items-center justify-between gap-4">
            <h2 className="text-base font-semibold">{title}</h2>
            <IconButton label="Close" onClick={onClose}>
              <X size={16} />
            </IconButton>
          </div>
          {children}
        </div>
      )}
    </dialog>
  );
}

export function ConfirmDialog({
  open,
  title,
  body,
  action,
  onConfirm,
  onClose,
}: {
  open: boolean;
  title: string;
  body: string;
  action: string;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog title={title} open={open} onClose={onClose}>
      <p className="mb-5 text-ink-2">{body}</p>
      <div className="flex justify-end gap-2">
        <Button onClick={onClose}>Keep it</Button>
        <Button tone="danger" onClick={onConfirm}>
          {action}
        </Button>
      </div>
    </Dialog>
  );
}

export function PageHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <header className="mb-5 flex flex-wrap items-center justify-between gap-3">
      <h1 className="text-[22px] leading-tight font-semibold tracking-tight">{title}</h1>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </header>
  );
}

export function Section({
  title,
  actions,
  children,
  className = "",
}: {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={className}>
      <div className="mb-3 flex items-center justify-between gap-3 border-b border-rule pb-2">
        <h2 className="text-[13px] font-semibold text-ink">{title}</h2>
        {actions}
      </div>
      {children}
    </section>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <p className="py-6 text-[13px] text-ink-3">{children}</p>;
}

export function ErrorNote({ message }: { message: string }) {
  return (
    <p role="alert" className="rounded-md bg-danger-soft px-3 py-2 text-[13px] text-danger">
      {message}
    </p>
  );
}

export function ColorSwatch({ color, size = 10 }: { color: string | null; size?: number }) {
  return (
    <span
      aria-hidden
      className="inline-block shrink-0 rounded-[3px]"
      style={{ width: size, height: size, background: color ?? "var(--rule-strong)" }}
    />
  );
}
