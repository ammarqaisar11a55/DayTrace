import { ChevronDown, X, type LucideIcon } from "lucide-react";
import {
  useEffect,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type FocusEvent,
  type InputHTMLAttributes,
  type MouseEvent,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";

type ButtonTone = "primary" | "secondary" | "quiet" | "danger";

const BUTTON_TONES: Record<ButtonTone, string> = {
  primary: "bg-brand text-on-primary shadow-sm hover:brightness-110 active:brightness-95",
  secondary: "border border-line-strong bg-surface text-fg hover:bg-hover active:bg-line",
  quiet: "text-fg-2 hover:bg-hover hover:text-fg active:bg-line",
  danger: "border border-danger/50 text-danger hover:bg-danger-soft active:bg-danger-soft/70",
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
      className={`inline-flex h-9 items-center justify-center gap-1.5 rounded-md px-3.5 text-[13px] font-medium whitespace-nowrap transition duration-150 disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:brightness-100 ${BUTTON_TONES[tone]} ${className}`}
    />
  );
}

const EDGE = 96;

/**
 * A label shown on hover or keyboard focus. It is positioned against the
 * viewport, so scrolling panels and dialogs never clip it.
 */
export function Tooltip({
  label,
  side = "top",
  children,
}: {
  label: string;
  side?: "top" | "bottom";
  children: ReactNode;
}) {
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const show = (event: MouseEvent<HTMLElement> | FocusEvent<HTMLElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setAnchor(rect), 350);
  };
  const hide = () => {
    window.clearTimeout(timer.current);
    setAnchor(null);
  };

  let position: CSSProperties = {};
  if (anchor) {
    const center = anchor.left + anchor.width / 2;
    const top = side === "bottom" ? anchor.bottom + 6 : anchor.top - 6;
    const lift = side === "top" ? "-100%" : "0";
    if (center > window.innerWidth - EDGE) {
      position = { top, right: Math.max(window.innerWidth - anchor.right, 4), transform: `translateY(${lift})` };
    } else if (center < EDGE) {
      position = { top, left: Math.max(anchor.left, 4), transform: `translateY(${lift})` };
    } else {
      position = { top, left: center, transform: `translate(-50%, ${lift})` };
    }
  }

  return (
    <span className="inline-flex" onMouseEnter={show} onMouseLeave={hide} onFocus={show} onBlur={hide} onMouseDown={hide}>
      {children}
      {anchor && (
        <span
          aria-hidden
          className="pointer-events-none fixed z-50 rounded-sm border border-line bg-elevated px-2 py-1 text-xs whitespace-nowrap text-fg shadow-md"
          style={position}
        >
          {label}
        </span>
      )}
    </span>
  );
}

export function IconButton({
  label,
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <Tooltip label={label}>
      <button
        type="button"
        aria-label={label}
        {...props}
        className={`inline-flex size-8 items-center justify-center rounded-sm text-fg-2 transition-colors duration-150 hover:bg-hover hover:text-fg active:bg-line disabled:cursor-not-allowed disabled:text-fg-disabled disabled:hover:bg-transparent ${className}`}
      />
    </Tooltip>
  );
}

const FIELD_CLASS =
  "h-9 rounded-md border border-line-strong bg-surface px-3 text-[13px] text-fg transition-colors duration-150 placeholder:text-fg-3 hover:border-fg-3/60 focus:border-accent focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/25 disabled:cursor-not-allowed disabled:opacity-50";

/** Fields fill their container unless the caller gives them a width. */
export function Input({ className = "w-full", ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`${FIELD_CLASS} ${className}`} />;
}

/** The width class applies to the wrapper, which also draws the chevron. */
export function Select({ className = "w-full", ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <span className={`relative inline-flex ${className}`}>
      <select {...props} className={`${FIELD_CLASS} w-full appearance-none pr-8`} />
      <ChevronDown
        aria-hidden
        size={14}
        className="pointer-events-none absolute top-1/2 right-2.5 -translate-y-1/2 text-fg-3"
      />
    </span>
  );
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[13px] font-medium text-fg">{label}</span>
      {children}
      {hint && <span className="mt-1.5 block text-xs text-fg-3">{hint}</span>}
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
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-45 ${
        checked ? "bg-brand" : "bg-line-strong hover:bg-fg-3/50"
      }`}
    >
      <span
        className={`absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow-sm transition-transform duration-150 ${
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
      className={`w-[calc(100vw-48px)] rounded-xl border border-line bg-elevated p-0 shadow-md ${
        wide ? "max-w-2xl" : "max-w-md"
      }`}
    >
      {open && (
        <div className="relative p-6">
          <h2 className="mb-5 pr-10 text-base leading-8 font-semibold">{title}</h2>
          {children}
          {/* Last in order, so opening a dialog focuses its first field instead. */}
          <span className="absolute top-6 right-5">
            <IconButton label="Close" onClick={onClose}>
              <X size={16} />
            </IconButton>
          </span>
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
      <p className="mb-6 text-[13px] text-fg-2">{body}</p>
      <div className="flex justify-end gap-2">
        <Button onClick={onClose}>Keep it</Button>
        <Button tone="danger" onClick={onConfirm}>
          {action}
        </Button>
      </div>
    </Dialog>
  );
}

export function PageHeader({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <header className="mb-6 flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
      <div className="min-w-0">
        <h1 className="text-[28px] leading-tight font-semibold tracking-tight">{title}</h1>
        {description && <div className="mt-1 text-[13px] text-fg-3">{description}</div>}
      </div>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </header>
  );
}

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`rounded-lg border border-line bg-surface shadow-sm ${className}`}>{children}</div>;
}

/** A titled card. Pages are built from these. */
export function Section({
  title,
  description,
  actions,
  children,
  className = "",
}: {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`min-w-0 rounded-lg border border-line bg-surface p-5 shadow-sm ${className}`}>
      <div className="mb-4 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-[15px] leading-snug font-semibold text-fg">{title}</h2>
          {description && <p className="mt-0.5 text-xs text-fg-3">{description}</p>}
        </div>
        {actions}
      </div>
      <div>{children}</div>
    </section>
  );
}

type BadgeTone = "neutral" | "primary" | "success" | "warn" | "danger";

const BADGE_TONES: Record<BadgeTone, string> = {
  neutral: "bg-hover text-fg-2",
  primary: "bg-primary-soft text-accent",
  success: "bg-success/15 text-success",
  warn: "bg-warn-soft text-warn",
  danger: "bg-danger-soft text-danger",
};

export function Badge({ tone = "neutral", children }: { tone?: BadgeTone; children: ReactNode }) {
  return (
    <span className={`inline-flex h-5 items-center gap-1 rounded-full px-2 text-[11px] font-medium whitespace-nowrap ${BADGE_TONES[tone]}`}>
      {children}
    </span>
  );
}

export function Empty({ children, icon: Icon }: { children: ReactNode; icon?: LucideIcon }) {
  return (
    <div className="flex flex-col items-center gap-2.5 py-8 text-center">
      {Icon && (
        <span className="inline-flex size-9 items-center justify-center rounded-md bg-primary-soft text-accent">
          <Icon size={17} strokeWidth={1.75} />
        </span>
      )}
      <p className="max-w-sm text-[13px] text-fg-3">{children}</p>
    </div>
  );
}

export function ErrorNote({ message }: { message: string }) {
  return (
    <p role="alert" className="mb-4 rounded-md border border-danger/30 bg-danger-soft px-3.5 py-2.5 text-[13px] text-danger">
      {message}
    </p>
  );
}

export function ColorSwatch({ color, size = 10 }: { color: string | null; size?: number }) {
  return (
    <span
      aria-hidden
      className="inline-block shrink-0 rounded-[3px]"
      style={{ width: size, height: size, background: color ?? "var(--dt-border-strong)" }}
    />
  );
}
