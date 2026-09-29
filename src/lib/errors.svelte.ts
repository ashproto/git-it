// The queue behind the error popup (ErrorDialog.svelte), and the two ways to add to it.
// Deliberately separate from the single-slot `dialogs`: opening a dialog there settles
// whichever one is pending as cancelled, so an error arriving mid-prompt would cancel it.
import { errorText, summarizeGitOutput } from "./errorReport";
import { appState } from "./store.svelte";

export type ErrorReport = {
  id: number;
  kind: "error" | "notice";
  title: string;
  headline: string;
  hints: string;
  output: string;
};

let queue = $state<ErrorReport[]>([]);
let last = $state<ErrorReport | null>(null);
let nextId = 1;

function enqueue(report: ErrorReport): void {
  const tail = queue[queue.length - 1];
  // A failure reported twice in a row shows once.
  if (
    tail &&
    tail.kind === report.kind &&
    tail.title === report.title &&
    tail.headline === report.headline &&
    tail.output === report.output
  ) {
    return;
  }
  queue = [...queue, report];
}

export const errors = {
  /** The report on screen, or null when the popup is closed. */
  get current(): ErrorReport | null {
    return queue[0] ?? null;
  },
  /** Reports queued, counting the one on screen. */
  get pending(): number {
    return queue.length;
  },
  /** The most recent error (never a notice), kept after dismissal so the status bar can reopen it. */
  get last(): ErrorReport | null {
    return last;
  },
  dismiss(): void {
    queue = queue.slice(1);
  },
  reopenLast(): void {
    if (last) enqueue({ ...last, id: nextId++ });
  },
  /** Tests only. */
  reset(): void {
    queue = [];
    last = null;
  },
};

/**
 * Report a failure: git's real error goes to the status bar (red) and the Output log, and a
 * popup opens unless `quiet` (background work the user didn't start). A wrapper that returns
 * false has already reported; its caller must not report again.
 */
export function reportError(title: string, error: unknown, opts: { quiet?: boolean } = {}): void {
  const { headline, hints, output } = summarizeGitOutput(errorText(error));
  const report: ErrorReport = { id: nextId++, kind: "error", title, headline, hints, output };
  last = report;
  appState.setErrorStatus(`${title}: ${headline.split("\n")[0]}`);
  appState.appendLog(`[error] ${title}\n${output}`);
  if (!opts.quiet) enqueue(report);
}

/**
 * Explain why a click did nothing. Not an error: the status bar stays its normal colour.
 */
export function reportNotice(title: string, message: string): void {
  appState.status = message;
  enqueue({ id: nextId++, kind: "notice", title, headline: message, hints: "", output: "" });
}
