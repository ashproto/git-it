// Window drag handler for Tauri 2. We don't use `data-tauri-drag-region`
// because it's broken in v2 (tauri-apps/tauri#9901). Instead we attach a
// `mousedown` handler that calls startDragging() synchronously on the same
// event tick — AppKit needs that to hand the drag off to the window server.
//
// Skips clicks on interactive descendants so buttons/inputs inside the drag
// region keep working.

import { getCurrentWindow } from "@tauri-apps/api/window";

const INTERACTIVE_SELECTOR =
  'button, a, input, select, textarea, label, [role="button"], ' +
  '[role="menuitem"], [role="menuitemradio"], [role="menuitemcheckbox"], ' +
  '[role="switch"], [contenteditable="true"], [data-no-drag]';

function isTauriShell(): boolean {
  return (
    typeof document !== "undefined" &&
    document.documentElement.dataset.tauri === "true"
  );
}

export function onWindowDragMouseDown(e: MouseEvent): void {
  if (!isTauriShell()) return;
  if (e.button !== 0) return; // primary button only
  const target = e.target as HTMLElement | null;
  if (target?.closest(INTERACTIVE_SELECTOR)) return;
  const win = getCurrentWindow();
  if (e.detail === 2) {
    // Native double-click-to-zoom on the titlebar
    void win.toggleMaximize().catch(() => {});
  } else {
    void win.startDragging().catch(() => {});
  }
}
