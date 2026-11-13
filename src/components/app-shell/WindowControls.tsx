import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Tooltip } from "../ui";

/** Follows the native window, so the button never disagrees with the real state. */
function useMaximized(): boolean {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const window = getCurrentWindow();
    let current = true;
    const sync = () =>
      void window.isMaximized().then((value) => {
        if (current) setMaximized(value);
      });
    sync();
    const unlisten = window.onResized(sync);
    return () => {
      current = false;
      void unlisten.then((stop) => stop());
    };
  }, []);
  return maximized;
}

function Control({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip label={label} side="bottom">
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className={`inline-flex h-11 w-[46px] items-center justify-center text-fg-2 transition-colors duration-150 focus-visible:-outline-offset-2 disabled:text-fg-disabled ${
          danger
            ? "hover:bg-close hover:text-white active:bg-close-pressed"
            : "hover:bg-hover hover:text-fg active:bg-line-strong"
        }`}
      >
        {children}
      </button>
    </Tooltip>
  );
}

export function WindowControls() {
  const maximized = useMaximized();
  const window = getCurrentWindow();
  return (
    <div className="flex h-full shrink-0">
      <Control label="Minimize" onClick={() => void window.minimize()}>
        <Minus size={15} strokeWidth={1.5} />
      </Control>
      <Control label={maximized ? "Restore" : "Maximize"} onClick={() => void window.toggleMaximize()}>
        {maximized ? (
          <Copy size={13} strokeWidth={1.5} className="-scale-x-100" />
        ) : (
          <Square size={12} strokeWidth={1.5} />
        )}
      </Control>
      <Control label="Close" danger onClick={() => void window.close()}>
        <X size={16} strokeWidth={1.5} />
      </Control>
    </div>
  );
}
