import { Logo } from "./Logo";
import { WindowControls } from "./WindowControls";

/**
 * Replaces the system title bar. Empty space drags the window and a double
 * click maximizes it; Tauri skips buttons inside a "deep" drag region, and it
 * handles resizing at the window edges itself.
 */
export function TitleBar() {
  return (
    <header data-tauri-drag-region="deep" className="flex h-11 shrink-0 items-center border-b border-line bg-sidebar">
      <div className="flex w-60 shrink-0 items-center gap-2.5 px-4">
        <Logo size={20} />
        <span className="text-[13px] font-semibold tracking-tight text-fg">DayTrace</span>
      </div>
      <div className="h-full min-w-0 flex-1" />
      <WindowControls />
    </header>
  );
}
