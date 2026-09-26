# Error Reporting Core Implementation Plan (PR 1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When an operation fails, a popup shows git's actual error sentence, git's own hints, and the raw output. The status bar turns red and reopens the popup. No error is visible only as a truncated status line any more.

**Architecture:**
- A pure module (`errorReport.ts`) turns any rejection into `{ headline, hints, output }`.
- A small runes store (`errors.svelte.ts`) queues reports separately from the single-slot `dialogs`, so a failure never cancels an open prompt. It also exposes the two entry points, `reportError` and `reportNotice`.
- `ErrorDialog.svelte` renders the head of the queue above every other modal.
- Every `gitActions` wrapper routes its failures through `reportError`, and the old `dialogs.alert` is deleted.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, Vitest 4 (node env by default; jsdom per file), Tauri 2 (untouched by this PR).

**Spec:** `docs/superpowers/specs/2026-09-26-error-reporting-and-pull-dialog-design.md`, Part A.
**Branch:** `feat/error-reporting-core` (already created off `next`; the spec is committed on it).

## Global Constraints

- **Gates:** `npm run check` must report 0 errors *and* 0 warnings (the baseline is 0/0), then `npm test`, `cargo test`, and `cargo clippy -p git-core -p git-it -- -D warnings` (the exact CI command).
- **Commits** follow `@commitlint/config-conventional`:
  - type prefix (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`)
  - subject lowercase-first, ≤100 characters
  - body lines ≤100 characters
  - every message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
  - use a heredoc for multi-line messages
- **Colour:** error colour is `--danger` (red). Never use `--err` for errors; in this codebase it is the amber *warning* token. Notice (ⓘ) colour is `--accent`.
- **Stacking:** the error popup overlay is `z-index: 4100`, one step above `Modal.svelte`'s dialog layer (`4000`).
- **Ownership rule:** a `gitActions` wrapper that returns `false` has already reported its failure. Callers never report it again.
- **Selectable text:** the body defaults to `user-select: none`. Text the user may copy gets the `selectable` class.
- **Tests:**
  - Assert on observable state: `errors.current`, `errors.pending`, `errors.last`, `appState.status`, `appState.statusIsError`.
  - Never `vi.spyOn` `reportError`/`reportNotice`; ESM bindings can't be spied. `vi.mock` of the whole module is fine.
  - Call `errors.reset()` in `afterEach`.
- **Style:** surgical changes that match the surrounding style and comment density. Remove only what these changes orphan.
- **Tauri-only paths:** vitest and the browser preview cannot exercise them. The real-app check in Task 7 covers them.

## File Structure

| File | Responsibility |
|---|---|
| `src/lib/errorReport.ts` (new) | Pure: `errorText(e)`, `summarizeGitOutput(text)`. |
| `src/lib/errorReport.test.ts` (new) | Fixture tests using real captured git output. |
| `src/lib/errors.svelte.ts` (new) | Report queue (`errors`), `reportError`, `reportNotice`. |
| `src/lib/errors.test.ts` (new) | Queue, dedupe, quiet, last/reopen, status and log side effects. |
| `src/lib/store.svelte.ts` | Adds `statusIsError` and `setErrorStatus`; the `status` setter clears the flag. |
| `src/lib/components/LogPanel.svelte` | Styles `[error]` log entries like `[stderr]` ones. |
| `src/lib/components/ErrorDialog.svelte` (new) | The popup. |
| `src/lib/components/ErrorDialog.test.ts` (new) | jsdom component tests. |
| `src/routes/+page.svelte` | Mounts `<ErrorDialog />`. |
| `src/lib/overlays.ts` | `anyOverlayOpen()` counts the error popup. |
| `src/lib/components/StatusBar.svelte` | A failed status becomes a red button that reopens the error. |
| `src/lib/components/StatusBar.test.ts` (new) | jsdom component tests. |
| `src/lib/gitActions.ts` | Wrappers, `runRemote` rules, `reloadGraph`, `amendCommit`, `squashMerge` and `cancelRemote` report through `reportError`. Deletes the local `errorText`/`firstLine`. |
| `src/lib/gitActions.errorReporting.test.ts` (new) | Wrapper-level behaviour. |
| `src/lib/gitActions.partialFailure.test.ts` | `afterEach` hygiene. |
| `src/lib/dialogs.svelte.ts`, `src/lib/components/Modal.svelte` | Delete the `alert` dialog kind. |
| `src/lib/repositoryFlows.ts`, `src/lib/repositoryFlows.test.ts` | Switch from `dialogs.alert` to `reportError`. |

---

### Task 1: Summarize git output into a headline and hints

**Files:**
- Create: `src/lib/errorReport.ts`
- Test: `src/lib/errorReport.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `export type GitErrorSummary = { headline: string; hints: string; output: string }`
  - `export function errorText(e: unknown): string`
  - `export function summarizeGitOutput(text: string): GitErrorSummary`

> **Execution note (learning mode):** the continuation rule inside `pickHeadline`, i.e. which lines after the first `fatal:`/`error:` line still belong to the headline, is offered to the user to write (about 6 lines). The tests below are its contract. If the user declines, use the implementation in Step 3 as-is.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/errorReport.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { errorText, summarizeGitOutput } from "./errorReport";

// Real git 2.54 output (LC_ALL=C) captured from scratch repositories; paths shortened.
const DIVERGENT_PULL = [
  "From /tmp/origin",
  "   d728e86..b362e35  main       -> origin/main",
  "hint: You have divergent branches and need to specify how to reconcile them.",
  "hint: You can do so by running one of the following commands sometime before",
  "hint: your next pull:",
  "hint:",
  "hint:   git config pull.rebase false  # merge",
  "hint:   git config pull.rebase true   # rebase",
  "hint:   git config pull.ff only       # fast-forward only",
  "hint:",
  'hint: You can replace "git config" with "git config --global" to set a default',
  "hint: preference for all repositories. You can also pass --rebase, --no-rebase,",
  "hint: or --ff-only on the command line to override the configured default per",
  "hint: invocation.",
  "fatal: Need to specify how to reconcile divergent branches.",
  "",
].join("\n");

const FF_ONLY_PULL = [
  "From /tmp/origin",
  "   89382b4..6efa90e  main       -> origin/main",
  "hint: Diverging branches can't be fast-forwarded, you need to either:",
  "hint:",
  "hint: \tgit merge --no-ff",
  "hint:",
  "hint: or:",
  "hint:",
  "hint: \tgit rebase",
  "hint:",
  'hint: Disable this message with "git config set advice.diverging false"',
  "fatal: Not possible to fast-forward, aborting.",
  "",
].join("\n");

// Git It reads stdout and stderr on separate threads, so stdout lines ("Updating …") can land
// anywhere in the combined text. This is the order one real run produced.
const OVERWRITTEN_PULL = [
  "error: Your local changes to the following files would be overwritten by merge:",
  "\tf",
  "Please commit your changes or stash them before you merge.",
  "Aborting",
  "Updating 89382b4..6efa90e",
  "",
].join("\n");

const REJECTED_PUSH = [
  "To /tmp/origin.git",
  " ! [rejected]        main -> main (non-fast-forward)",
  "error: failed to push some refs to '/tmp/origin.git'",
  "hint: Updates were rejected because the tip of your current branch is behind",
  "hint: its remote counterpart. If you want to integrate the remote changes,",
  "hint: use 'git pull' before pushing again.",
  "hint: See the 'Note about fast-forwards' in 'git push --help' for details.",
  "",
].join("\n");

const OVERWRITTEN_HEADLINE = [
  "Your local changes to the following files would be overwritten by merge:",
  "\tf",
  "Please commit your changes or stash them before you merge.",
].join("\n");

describe("summarizeGitOutput", () => {
  it("leads a divergent pull with git's fatal line, not the fetch header", () => {
    const summary = summarizeGitOutput(DIVERGENT_PULL);
    expect(summary.headline).toBe("Need to specify how to reconcile divergent branches.");
    expect(summary.hints).toBe(
      [
        "You have divergent branches and need to specify how to reconcile them.",
        "You can do so by running one of the following commands sometime before",
        "your next pull:",
        "",
        "  git config pull.rebase false  # merge",
        "  git config pull.rebase true   # rebase",
        "  git config pull.ff only       # fast-forward only",
        "",
        'You can replace "git config" with "git config --global" to set a default',
        "preference for all repositories. You can also pass --rebase, --no-rebase,",
        "or --ff-only on the command line to override the configured default per",
        "invocation.",
      ].join("\n"),
    );
    expect(summary.output).toBe(DIVERGENT_PULL);
  });

  it("drops git's advice about silencing a hint", () => {
    const summary = summarizeGitOutput(FF_ONLY_PULL);
    expect(summary.headline).toBe("Not possible to fast-forward, aborting.");
    expect(summary.hints).toBe(
      "Diverging branches can't be fast-forwarded, you need to either:\n\n\tgit merge --no-ff\n\nor:\n\n\tgit rebase",
    );
  });

  it("keeps an error's file list and follow-up sentence, not Aborting or stdout", () => {
    const summary = summarizeGitOutput(OVERWRITTEN_PULL);
    expect(summary.headline).toBe(OVERWRITTEN_HEADLINE);
    expect(summary.hints).toBe("");
  });

  it("skips stdout lines that interleave inside the error block", () => {
    const interleaved = [
      "error: Your local changes to the following files would be overwritten by merge:",
      "Updating 89382b4..6efa90e",
      "\tf",
      "Please commit your changes or stash them before you merge.",
      "Aborting",
    ].join("\n");
    expect(summarizeGitOutput(interleaved).headline).toBe(OVERWRITTEN_HEADLINE);
  });

  it("ignores push ref-status lines and keeps the push hints", () => {
    const summary = summarizeGitOutput(REJECTED_PUSH);
    expect(summary.headline).toBe("failed to push some refs to '/tmp/origin.git'");
    expect(summary.hints).toBe(
      [
        "Updates were rejected because the tip of your current branch is behind",
        "its remote counterpart. If you want to integrate the remote changes,",
        "use 'git pull' before pushing again.",
        "See the 'Note about fast-forwards' in 'git push --help' for details.",
      ].join("\n"),
    );
  });

  it("leads with the first error when git reports several", () => {
    const summary = summarizeGitOutput(
      "error: cannot rebase: You have unstaged changes.\nerror: Please commit or stash them.",
    );
    expect(summary.headline).toBe("cannot rebase: You have unstaged changes.");
  });

  it("passes a one-line non-git message through", () => {
    expect(summarizeGitOutput("A remote operation is already in progress.").headline).toBe(
      "A remote operation is already in progress.",
    );
  });

  it("falls back to the last meaningful line when nothing is prefixed", () => {
    expect(summarizeGitOutput("first line\nsecond line\n").headline).toBe("second line");
    expect(
      summarizeGitOutput("From /tmp/origin\nmerge: nope - not something we can merge\n").headline,
    ).toBe("merge: nope - not something we can merge");
  });

  it("says so when there is no message at all", () => {
    expect(summarizeGitOutput("").headline).toBe("The operation failed without an error message.");
    expect(summarizeGitOutput("fatal:").headline).toBe(
      "The operation failed without an error message.",
    );
  });

  it("handles Windows line endings", () => {
    expect(summarizeGitOutput("From x\r\nfatal: boom\r\n").headline).toBe("boom");
  });
});

describe("errorText", () => {
  it("unwraps GitHub error objects", () => {
    expect(errorText({ kind: "Other", message: "name already exists" })).toBe("name already exists");
    expect(errorText({ kind: "NotAuthed" })).toBe("NotAuthed");
  });

  it("uses an Error's message", () => {
    expect(errorText(new Error("boom"))).toBe("boom");
  });

  it("maps null and undefined to empty text", () => {
    expect(errorText(null)).toBe("");
    expect(errorText(undefined)).toBe("");
  });

  it("stringifies anything else", () => {
    expect(errorText("fatal: x")).toBe("fatal: x");
    expect(errorText(42)).toBe("42");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/errorReport.test.ts`
Expected: FAIL, because `./errorReport` doesn't exist ("Failed to resolve import").

- [ ] **Step 3: Implement**

Create `src/lib/errorReport.ts`:

```ts
// Turn whatever an operation rejected with into something a person can read: the sentence
// git actually complained with, git's own advice, and the raw output for anyone who wants
// the whole story. Pure (Tauri-free) so it is unit-tested with vitest.

export type GitErrorSummary = {
  /** git's complaint: the first `fatal:`/`error:` line plus its continuation lines. */
  headline: string;
  /** git's `hint:` advice without the prefix; "" when there is none. */
  hints: string;
  /** The text exactly as received. */
  output: string;
};

const NO_MESSAGE = "The operation failed without an error message.";

// Full error text. GitHub commands reject with a serialized GithubError OBJECT
// ({kind, message?}) — String() on that yields "[object Object]", so unwrap it first.
export function errorText(e: unknown): string {
  if (e == null) return "";
  if (typeof e === "object" && "kind" in e) {
    const g = e as { kind: string; message?: string };
    return String(g.kind === "Other" ? (g.message ?? g.kind) : g.kind);
  }
  if (e instanceof Error) return e.message;
  return String(e);
}

const HEADLINE = /^(?:fatal|error): ?/;
const PREFIXED = /^(?:fatal|error|warning|hint|remote):/;
const HINT = /^hint:(?: |$)/;

// Lines git writes to STDOUT during a pull or push. Git It reads stdout and stderr on separate
// threads, so these can land anywhere — even between an `error:` line and its file list.
function isStdoutNoise(line: string): boolean {
  return (
    /^Updating [0-9a-f]+\.\.[0-9a-f]+$/.test(line) || // merge preamble
    /^(?:From|To) \S/.test(line) || // fetch / push header
    line.startsWith(" ") // ref updates: "   a1b2..c3d4  main -> origin/main", " ! [rejected] …"
  );
}

function pickHeadline(lines: string[]): string | null {
  const start = lines.findIndex((line) => HEADLINE.test(line));
  if (start >= 0) {
    const block = [lines[start].replace(HEADLINE, "")];
    for (const line of lines.slice(start + 1)) {
      // The complaint ends at git's next prefixed line, its closing "Aborting", or a blank line.
      if (PREFIXED.test(line) || line === "Aborting" || line.trim() === "") break;
      if (isStdoutNoise(line)) continue;
      block.push(line); // tab-indented file lists and unprefixed follow-up sentences
    }
    return block.join("\n");
  }
  // Nothing prefixed (a non-git message, or git under a translated locale): the last line
  // that says something is the best guess.
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i];
    if (
      line.trim() &&
      !isStdoutNoise(line) &&
      !HINT.test(line) &&
      !line.startsWith("remote:") &&
      line !== "Aborting"
    ) {
      return line.trim();
    }
  }
  return null;
}

function collectHints(lines: string[]): string {
  const hints = lines
    .filter((line) => HINT.test(line))
    .map((line) => line.replace(HINT, ""))
    // CLI advice about silencing the hint means nothing in a GUI.
    .filter((line) => !line.startsWith("Disable this message with"));
  while (hints.length && hints[0].trim() === "") hints.shift();
  while (hints.length && hints[hints.length - 1].trim() === "") hints.pop();
  return hints.join("\n");
}

export function summarizeGitOutput(text: string): GitErrorSummary {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const headline = pickHeadline(lines);
  return {
    headline: headline && headline.trim() ? headline : NO_MESSAGE,
    hints: collectHints(lines),
    output: text,
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/lib/errorReport.test.ts`
Expected: PASS (14 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/errorReport.ts src/lib/errorReport.test.ts
git commit -F - <<'EOF'
feat: summarize git errors into a headline and hints

The status bar showed the first line of a failed op's output, which for
a pull is the fetch header. Pick git's own fatal/error sentence instead,
keep its hint lines as advice, and keep the raw text for details.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 2: Queue error reports and mark a failed status

**Files:**
- Create: `src/lib/errors.svelte.ts`
- Modify: `src/lib/store.svelte.ts` (the `status` state and its getter/setter)
- Modify: `src/lib/components/LogPanel.svelte` (the `.line` class binding)
- Test: `src/lib/errors.test.ts`

**Interfaces:**
- Consumes: `errorText`, `summarizeGitOutput` (Task 1).
- Produces:
  ```ts
  export type ErrorReport = { id: number; kind: "error" | "notice"; title: string; headline: string; hints: string; output: string };
  export const errors: {
    readonly current: ErrorReport | null; readonly pending: number; readonly last: ErrorReport | null;
    dismiss(): void; reopenLast(): void; reset(): void;
  };
  export function reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void;
  export function reportNotice(title: string, message: string): void;
  // store: appState.statusIsError (getter), appState.setErrorStatus(text: string)
  ```

- [ ] **Step 1: Write the failing tests**

Create `src/lib/errors.test.ts`:

```ts
import { afterEach, describe, expect, it } from "vitest";
import { errors, reportError, reportNotice } from "./errors.svelte";
import { appState } from "./store.svelte";

const DIVERGENT = "From /tmp/origin\nfatal: Need to specify how to reconcile divergent branches.\n";
const OVERWRITTEN =
  "error: Your local changes to the following files would be overwritten by merge:\n\tf\nAborting\n";

afterEach(() => {
  errors.reset();
  appState.status = "Ready";
  appState.clearLog();
});

describe("reportError", () => {
  it("queues a popup led by git's real error", () => {
    reportError("Pull failed", DIVERGENT);
    expect(errors.current).toMatchObject({
      kind: "error",
      title: "Pull failed",
      headline: "Need to specify how to reconcile divergent branches.",
      output: DIVERGENT,
    });
    expect(errors.pending).toBe(1);
  });

  it("puts the real error in the status bar, in the error state", () => {
    reportError("Pull failed", DIVERGENT);
    expect(appState.status).toBe("Pull failed: Need to specify how to reconcile divergent branches.");
    expect(appState.statusIsError).toBe(true);
  });

  it("uses only the headline's first line in the status bar", () => {
    reportError("Pull failed", OVERWRITTEN);
    expect(appState.status).toBe(
      "Pull failed: Your local changes to the following files would be overwritten by merge:",
    );
  });

  it("logs the full output", () => {
    reportError("Pull failed", DIVERGENT);
    expect(appState.logLines[appState.logLines.length - 1]).toBe(`[error] Pull failed\n${DIVERGENT}`);
  });

  it("shows queued reports in order", () => {
    reportError("Pull failed", DIVERGENT);
    reportError("Push failed", "error: failed to push some refs to 'origin'");
    expect(errors.pending).toBe(2);
    expect(errors.current?.title).toBe("Pull failed");
    errors.dismiss();
    expect(errors.current?.title).toBe("Push failed");
    errors.dismiss();
    expect(errors.current).toBeNull();
  });

  it("drops a report identical to the last one queued", () => {
    reportError("Pull failed", DIVERGENT);
    reportError("Pull failed", DIVERGENT);
    expect(errors.pending).toBe(1);
  });

  it("records a quiet report without a popup", () => {
    reportError("Update check failed", "offline", { quiet: true });
    expect(errors.current).toBeNull();
    expect(errors.last?.title).toBe("Update check failed");
    expect(appState.statusIsError).toBe(true);
  });

  it("reopens the last error after it was dismissed", () => {
    reportError("Pull failed", DIVERGENT);
    errors.dismiss();
    errors.reopenLast();
    expect(errors.current?.title).toBe("Pull failed");
  });

  it("gives a reopened report a fresh id", () => {
    reportError("Pull failed", DIVERGENT);
    const first = errors.current?.id;
    errors.dismiss();
    errors.reopenLast();
    expect(errors.current?.id).not.toBe(first);
  });

  it("clears the error state on the next ordinary status message", () => {
    reportError("Pull failed", DIVERGENT);
    appState.status = "Fetch — done.";
    expect(appState.statusIsError).toBe(false);
  });
});

describe("reportNotice", () => {
  it("queues an info popup without touching the error state", () => {
    reportNotice("Can't reword this commit", "Can't reword a merge commit here.");
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't reword this commit",
      headline: "Can't reword a merge commit here.",
      hints: "",
      output: "",
    });
    expect(appState.status).toBe("Can't reword a merge commit here.");
    expect(appState.statusIsError).toBe(false);
    expect(errors.last).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/errors.test.ts`
Expected: FAIL, because `./errors.svelte` doesn't exist.

- [ ] **Step 3: Add the error flag to the store**

In `src/lib/store.svelte.ts`, inside `makeState()`, directly below `let status = $state("Ready");`, add:

```ts
  // True while `status` holds a failure: the status bar paints it red and makes it a button
  // that reopens the error. Any ordinary `status` write clears it.
  let statusIsError = $state(false);
```

Replace the existing status accessors:

```ts
    get status() {
      return status;
    },
    set status(v: string) {
      status = v;
    },
```

with:

```ts
    get status() {
      return status;
    },
    set status(v: string) {
      status = v;
      statusIsError = false;
    },
    get statusIsError() {
      return statusIsError;
    },
    setErrorStatus(v: string) {
      status = v;
      statusIsError = true;
    },
```

- [ ] **Step 4: Create the report store**

Create `src/lib/errors.svelte.ts`:

```ts
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

/** Explain why a click did nothing. Not an error: the status bar stays its normal colour. */
export function reportNotice(title: string, message: string): void {
  appState.status = message;
  enqueue({ id: nextId++, kind: "notice", title, headline: message, hints: "", output: "" });
}
```

- [ ] **Step 5: Style `[error]` log entries like `[stderr]` ones**

In `src/lib/components/LogPanel.svelte`, replace:

```svelte
      <div class="line" class:err={line.startsWith("[stderr]")}>{line}</div>
```

with:

```svelte
      <div class="line" class:err={line.startsWith("[stderr]") || line.startsWith("[error]")}>{line}</div>
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npx vitest run src/lib/errors.test.ts`
Expected: PASS (11 tests).

- [ ] **Step 7: Commit**

```bash
git add src/lib/errors.svelte.ts src/lib/errors.test.ts src/lib/store.svelte.ts src/lib/components/LogPanel.svelte
git commit -F - <<'EOF'
feat: queue error reports and mark a failed status

Reports live in their own queue so a failure never cancels an open
dialog. reportError puts git's real error in a red status line and the
Output log; reportNotice explains a refused action without the red.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 3: The error popup

**Files:**
- Create: `src/lib/components/ErrorDialog.svelte`
- Modify: `src/routes/+page.svelte` (import and mount next to `<Modal />`)
- Modify: `src/lib/overlays.ts`
- Test: `src/lib/components/ErrorDialog.test.ts`

**Interfaces:**
- Consumes: `errors`, `reportError`, `reportNotice` (Task 2).
- Produces: `<ErrorDialog />` (no props). `anyOverlayOpen()` now also returns true while a report is on screen.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/ErrorDialog.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { errors, reportError, reportNotice } from "../errors.svelte";
import ErrorDialog from "./ErrorDialog.svelte";

const DIVERGENT = [
  "From /tmp/origin",
  "   d728e86..b362e35  main       -> origin/main",
  "hint: You have divergent branches and need to specify how to reconcile them.",
  "hint:   git config pull.rebase false  # merge",
  "fatal: Need to specify how to reconcile divergent branches.",
].join("\n");

let component: ReturnType<typeof mount> | null = null;

function render(): HTMLElement {
  const target = document.createElement("div");
  document.body.appendChild(target);
  component = mount(ErrorDialog, { target });
  return target;
}

afterEach(() => {
  if (component) unmount(component);
  component = null;
  errors.reset();
  Reflect.deleteProperty(navigator, "clipboard");
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

describe("ErrorDialog", () => {
  it("renders nothing until something is reported", async () => {
    const target = render();
    await tick();
    expect(target.querySelector("[role=alertdialog]")).toBeNull();
  });

  it("leads with git's real error and shows its hints", async () => {
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    expect(target.querySelector("h3")?.textContent).toBe("Pull failed");
    expect(target.querySelector(".headline-text")?.textContent).toBe(
      "Need to specify how to reconcile divergent branches.",
    );
    expect(target.querySelector(".hints")?.textContent).toContain("git config pull.rebase false");
    expect(target.querySelector(".glyph")?.textContent).toBe("⚠");
  });

  it("keeps the raw output behind a disclosure", async () => {
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    expect(target.querySelector(".output")).toBeNull();
    target.querySelector<HTMLButtonElement>(".disclosure")?.click();
    await tick();
    expect(target.querySelector(".output")?.textContent).toContain("From /tmp/origin");
  });

  it("offers no disclosure when the output is only the headline", async () => {
    const target = render();
    reportError("Push failed", "A remote operation is already in progress.");
    await tick();
    expect(target.querySelector(".disclosure")).toBeNull();
  });

  it("focuses OK and advances to the next queued report", async () => {
    const target = render();
    reportError("Pull failed", DIVERGENT);
    reportError("Push failed", "error: failed to push some refs to 'origin'");
    await tick();
    expect(target.querySelector(".count")?.textContent).toBe("1 of 2");
    const ok = target.querySelector<HTMLButtonElement>("button.primary");
    expect(document.activeElement).toBe(ok);
    ok?.click();
    await tick();
    expect(target.querySelector("h3")?.textContent).toBe("Push failed");
    expect(target.querySelector(".count")).toBeNull();
  });

  it("closes on Escape and returns focus to where it was", async () => {
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    target
      .querySelector(".overlay")
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await tick();
    expect(errors.current).toBeNull();
    expect(target.querySelector("[role=alertdialog]")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("keeps its keys from reaching window-level shortcuts", async () => {
    const target = render();
    const onWindowKey = vi.fn();
    window.addEventListener("keydown", onWindowKey);
    reportError("Pull failed", DIVERGENT);
    await tick();
    target
      .querySelector(".overlay")
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    window.removeEventListener("keydown", onWindowKey);
    expect(onWindowKey).not.toHaveBeenCalled();
  });

  it("shows a notice with an info glyph and no copy button", async () => {
    const target = render();
    reportNotice("Can't reword this commit", "Can't reword a merge commit here.");
    await tick();
    expect(target.querySelector(".glyph")?.textContent).toBe("ⓘ");
    expect(target.querySelector(".copy")).toBeNull();
  });

  it("copies the title, headline and raw output", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    const copy = target.querySelector<HTMLButtonElement>(".copy");
    copy?.click();
    await vi.waitFor(() => expect(copy?.textContent).toBe("Copied"));
    expect(writeText).toHaveBeenCalledWith(
      `Pull failed\nNeed to specify how to reconcile divergent branches.\n\n${DIVERGENT}`,
    );
  });

  it("says so when copying is unavailable", async () => {
    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    const copy = target.querySelector<HTMLButtonElement>(".copy");
    copy?.click();
    await vi.waitFor(() => expect(copy?.textContent).toBe("Copy failed"));
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/components/ErrorDialog.test.ts`
Expected: FAIL, because `./ErrorDialog.svelte` doesn't exist.

- [ ] **Step 3: Create the component**

Create `src/lib/components/ErrorDialog.svelte`:

```svelte
<script lang="ts">
  import { errors } from "../errors.svelte";

  let okEl = $state<HTMLButtonElement | undefined>();
  let showOutput = $state(false);
  let copyState = $state<"idle" | "copied" | "failed">("idle");
  // Plain bookkeeping for the effect below (never read by the template).
  let shownId: number | null = null;
  let focusedId: number | null = null;
  let returnFocusTo: HTMLElement | null = null;

  const report = $derived(errors.current);
  // A one-line error has nothing more to show than its headline.
  const hasOutput = $derived(
    !!report &&
      report.kind === "error" &&
      report.output.trim() !== "" &&
      report.output.trim() !== report.headline.trim(),
  );

  // Reset the disclosure and copy state per report, focus OK once the button exists, and give
  // focus back to wherever it was when the queue empties. `okEl` is read on every run, so if
  // the button binds after this effect first runs, the effect runs again and still focuses it.
  $effect(() => {
    const r = errors.current;
    if (!r) {
      if (shownId !== null) {
        const el = returnFocusTo;
        shownId = focusedId = null;
        returnFocusTo = null;
        if (el?.isConnected) el.focus();
      }
      return;
    }
    if (r.id !== shownId) {
      if (shownId === null) {
        const active = document.activeElement;
        returnFocusTo = active instanceof HTMLElement ? active : null;
      }
      shownId = r.id;
      showOutput = false;
      copyState = "idle";
    }
    if (okEl && focusedId !== r.id) {
      focusedId = r.id;
      okEl.focus();
    }
  });

  async function copyDetails() {
    const r = errors.current;
    if (!r) return;
    try {
      await navigator.clipboard.writeText(`${r.title}\n${r.headline}\n\n${r.output}`);
      copyState = "copied";
    } catch {
      copyState = "failed";
    }
  }

  function onKeydown(e: KeyboardEvent) {
    // Keys pressed while the popup is up are its alone: window-level Escape handlers
    // underneath (the GitHub action modal, a diff selection) must not fire too.
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      errors.dismiss();
    }
  }
</script>

{#if report}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay"
    onpointerdown={(e) => {
      if (e.target === e.currentTarget) errors.dismiss();
    }}
    onkeydown={onKeydown}
  >
    <div
      class="dialog"
      class:notice={report.kind === "notice"}
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="error-dialog-title"
      aria-describedby="error-dialog-headline"
    >
      <div class="title-row">
        <h3 id="error-dialog-title">{report.title}</h3>
        {#if errors.pending > 1}
          <span class="count">1 of {errors.pending}</span>
        {/if}
      </div>
      <div class="body">
        <div class="headline">
          <span class="glyph" aria-hidden="true">{report.kind === "error" ? "⚠" : "ⓘ"}</span>
          <p id="error-dialog-headline" class="headline-text selectable">{report.headline}</p>
        </div>
        {#if report.hints}
          <pre class="hints selectable">{report.hints}</pre>
        {/if}
        {#if hasOutput}
          <button
            type="button"
            class="disclosure"
            aria-expanded={showOutput}
            onclick={() => (showOutput = !showOutput)}
          >
            <span aria-hidden="true">{showOutput ? "▾" : "▸"}</span>
            {showOutput ? "Hide git output" : "Show git output"}
          </button>
          {#if showOutput}
            <pre class="output selectable">{report.output}</pre>
          {/if}
        {/if}
      </div>
      <div class="actions">
        {#if report.kind === "error"}
          <button type="button" class="copy" onclick={copyDetails}>
            {copyState === "copied" ? "Copied" : copyState === "failed" ? "Copy failed" : "Copy details"}
          </button>
        {/if}
        <button type="button" class="primary" bind:this={okEl} onclick={() => errors.dismiss()}>OK</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    /* One step above the dialog layer (Modal.svelte, 4000): an error that lands while a
       prompt is open covers it without closing it. */
    z-index: 4100;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .dialog {
    width: min(560px, calc(100vw - 32px));
    max-height: 80vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--popover-bg, var(--panel-bg));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 16px 18px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
  }
  .title-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin: 0 0 10px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .count {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  /* The text scrolls; the title and buttons stay put. */
  .body {
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    margin: 0 0 14px;
  }
  .headline {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .glyph {
    flex-shrink: 0;
    font-size: 14px;
    line-height: 1.5;
    color: var(--danger);
  }
  .notice .glyph {
    color: var(--accent);
  }
  .headline-text {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .hints {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    background: var(--btn-bg);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .output {
    margin: 6px 0 0;
    max-height: 260px;
    overflow: auto;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--input-bg);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.45;
    white-space: pre;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  button {
    padding: 6px 14px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .disclosure {
    margin-top: 10px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    font-size: 12px;
  }
  .disclosure:hover {
    background: none;
    color: var(--text);
  }
</style>
```

- [ ] **Step 4: Mount it and count it as an overlay**

In `src/routes/+page.svelte`, next to the existing `import Modal from "$lib/components/Modal.svelte";`, add:

```ts
  import ErrorDialog from "$lib/components/ErrorDialog.svelte";
```

and replace:

```svelte
  <Modal />
```

with:

```svelte
  <Modal />
  <ErrorDialog />
```

In `src/lib/overlays.ts`, add the import below `import { githubActions } from "./githubActions.svelte";`:

```ts
import { errors } from "./errors.svelte";
```

and replace:

```ts
    githubActions.pending !== null
  );
```

with:

```ts
    githubActions.pending !== null ||
    errors.current !== null
  );
```

- [ ] **Step 5: Run the tests and the type gate**

Run: `npx vitest run src/lib/components/ErrorDialog.test.ts`
Expected: PASS (10 tests).

Run: `npm run check`
Expected: `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/ErrorDialog.svelte src/lib/components/ErrorDialog.test.ts src/routes/+page.svelte src/lib/overlays.ts
git commit -F - <<'EOF'
feat: add an error popup above every dialog

Shows git's headline, its hints, and the raw output behind a disclosure,
with Copy details and OK. It sits one layer above the dialog stack so an
error never closes a prompt, and it keeps its keys from app shortcuts.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 4: A failed status reopens its error

**Files:**
- Modify: `src/lib/components/StatusBar.svelte`
- Test: `src/lib/components/StatusBar.test.ts`

**Interfaces:**
- Consumes: `errors.last`, `errors.reopenLast()`, `appState.statusIsError` (Task 2).
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/StatusBar.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { mount, tick, unmount } from "svelte";
import { errors, reportError } from "../errors.svelte";
import { appState } from "../store.svelte";
import StatusBar from "./StatusBar.svelte";

let component: ReturnType<typeof mount> | null = null;

function render(): HTMLElement {
  const target = document.createElement("div");
  document.body.appendChild(target);
  component = mount(StatusBar, { target });
  return target;
}

afterEach(() => {
  if (component) unmount(component);
  component = null;
  errors.reset();
  appState.status = "Ready";
  document.body.replaceChildren();
});

describe("StatusBar error state", () => {
  it("shows an ordinary message as plain text", async () => {
    appState.status = "Fetch — done.";
    const target = render();
    await tick();
    expect(target.querySelector("button.status-error")).toBeNull();
    expect(target.querySelector(".status-text")?.textContent).toBe("Fetch — done.");
  });

  it("turns a failure into a button that reopens the error", async () => {
    const target = render();
    reportError("Pull failed", "fatal: Need to specify how to reconcile divergent branches.");
    errors.dismiss();
    await tick();
    const button = target.querySelector<HTMLButtonElement>("button.status-error");
    expect(button?.textContent).toContain(
      "Pull failed: Need to specify how to reconcile divergent branches.",
    );
    button?.click();
    expect(errors.current?.title).toBe("Pull failed");
  });

  it("drops the error state on the next ordinary message", async () => {
    const target = render();
    reportError("Pull failed", "fatal: boom");
    appState.status = "Fetch — done.";
    await tick();
    expect(target.querySelector("button.status-error")).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/components/StatusBar.test.ts`
Expected: FAIL on the second test, because there is no `button.status-error`.

- [ ] **Step 3: Implement**

In `src/lib/components/StatusBar.svelte`, add the import below `import { appState } from "../store.svelte";`:

```ts
  import { errors } from "../errors.svelte";
```

Replace:

```svelte
    <span class="status-text" title={appState.status}>{appState.status}</span>
```

with:

```svelte
    {#if appState.statusIsError && errors.last}
      <button
        type="button"
        class="status-error"
        title="Show error details"
        onclick={() => errors.reopenLast()}
      >
        <span class="status-glyph" aria-hidden="true">⚠</span>
        <span class="status-text">{appState.status}</span>
      </button>
    {:else}
      <span class="status-text" title={appState.status}>{appState.status}</span>
    {/if}
```

In the same file's `<style>`, directly after the `.status-text { … }` rule, add:

```css
  /* A failure: red, and a button that reopens the full error in the popup. */
  .status-error {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--danger);
    font: inherit;
    cursor: pointer;
  }
  .status-error:hover .status-text {
    text-decoration: underline;
  }
  .status-glyph {
    flex-shrink: 0;
  }
```

- [ ] **Step 4: Run the tests and the type gate**

Run: `npx vitest run src/lib/components/StatusBar.test.ts`
Expected: PASS (3 tests).

Run: `npm run check`
Expected: 0 errors and 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/StatusBar.svelte src/lib/components/StatusBar.test.ts
git commit -F - <<'EOF'
feat: make a failed status reopen its error

A failure message in the status bar is red and clickable, so the full
error is one click away after the popup has been dismissed.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 5: Report every failed operation

**Files:**
- Modify: `src/lib/gitActions.ts`:
  - imports
  - `reloadGraph`'s load-failure branch
  - `run`, `runOp`, `runResolve`, `runWorktree`, `runDestructive`, `runDestructiveRebase`, `runRemote`
  - the local `errorText`/`firstLine` helpers
  - `gitActions.amendCommit`, `gitActions.squashMerge`, `gitActions.cancelRemote`
- Modify: `src/lib/gitActions.partialFailure.test.ts` (`afterEach`)
- Test: `src/lib/gitActions.errorReporting.test.ts`

**Interfaces:**
- Consumes: `reportError`, `errors` (Task 2); `errorText` (Task 1).
- Produces: every wrapper and direct path reports its own failures (the ownership rule). `gitActions.cancelRemote()` marks the in-flight remote op as cancelled by the user.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/gitActions.errorReporting.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { dialogs } from "./dialogs.svelte";
import { errors } from "./errors.svelte";
import { gitActions, reloadGraph } from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";
import { appState } from "./store.svelte";
import type { RemoteOutcome, RepoStatus } from "./types";

const REPO = "/tmp/git-it-error-test";

const repoStatus: RepoStatus = {
  head: { sha: SAMPLE_GRAPH[0].sha, branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};

const DIVERGENT = [
  "From /tmp/origin",
  "   d728e86..b362e35  main       -> origin/main",
  "hint: You have divergent branches and need to specify how to reconcile them.",
  "fatal: Need to specify how to reconcile divergent branches.",
].join("\n");

function outcome(patch: Partial<RemoteOutcome>): RemoteOutcome {
  return { ok: false, authFailed: false, conflicted: false, message: "", ...patch };
}

beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = REPO;
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH);
  vi.spyOn(api, "loadGraph").mockResolvedValue(SAMPLE_GRAPH);
  vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue([]);
  vi.spyOn(api, "remotes").mockResolvedValue([]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
});

afterEach(() => {
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("wrapper failures open the error popup", () => {
  it("run: checkout", async () => {
    vi.spyOn(api, "checkout").mockRejectedValue(
      "error: pathspec 'nope' did not match any file(s) known to git",
    );
    expect(await gitActions.checkout("nope")).toBe(false);
    expect(errors.current).toMatchObject({
      kind: "error",
      title: "Checkout nope failed",
      headline: "pathspec 'nope' did not match any file(s) known to git",
    });
    expect(appState.status).toBe(
      "Checkout nope failed: pathspec 'nope' did not match any file(s) known to git",
    );
    expect(appState.statusIsError).toBe(true);
  });

  it("runOp: merge", async () => {
    vi.spyOn(api, "merge").mockRejectedValue("merge: nope - not something we can merge");
    expect(await gitActions.merge("nope")).toBeNull();
    expect(errors.current).toMatchObject({
      title: "Merge nope failed",
      headline: "merge: nope - not something we can merge",
    });
  });

  it("runResolve: resolve a conflict", async () => {
    vi.spyOn(api, "resolveConflict").mockRejectedValue("error: path 'f' does not have our version");
    expect(await gitActions.resolveConflict("f", true)).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Resolve f failed",
      headline: "path 'f' does not have our version",
    });
  });

  it("runWorktree: apply a stash", async () => {
    vi.spyOn(api, "stashApply").mockRejectedValue(
      "error: Your local changes to the following files would be overwritten by merge:\n\tf\nAborting",
    );
    expect(await gitActions.stashApply(0)).toBe(false);
    expect(errors.current?.title).toBe("Apply stash@{0} failed");
    expect(errors.current?.headline).toBe(
      "Your local changes to the following files would be overwritten by merge:\n\tf",
    );
  });

  it("runDestructive: reset", async () => {
    vi.spyOn(dialogs, "confirmDestructive").mockResolvedValue({ confirmed: true, backup: false });
    vi.spyOn(api, "reset").mockRejectedValue("fatal: ambiguous argument 'abc': unknown revision");
    expect(await gitActions.reset("abc", "hard", "Moves main.")).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Reset (hard) failed",
      headline: "ambiguous argument 'abc': unknown revision",
    });
  });

  it("runDestructiveRebase: rebase onto", async () => {
    vi.spyOn(dialogs, "confirmDestructive").mockResolvedValue({ confirmed: true, backup: false });
    vi.spyOn(api, "rebase").mockRejectedValue(
      "error: cannot rebase: You have unstaged changes.\nerror: Please commit or stash them.",
    );
    expect(await gitActions.rebaseOnto("main", "Rewrites commits.")).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Rebase onto main failed",
      headline: "cannot rebase: You have unstaged changes.",
    });
  });

  it("amendCommit", async () => {
    vi.spyOn(api, "amend").mockRejectedValue(
      "fatal: You are in the middle of a merge -- cannot amend.",
    );
    expect(await gitActions.amendCommit("message")).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Amend commit failed",
      headline: "You are in the middle of a merge -- cannot amend.",
    });
  });

  it("squashMerge", async () => {
    vi.spyOn(api, "merge").mockRejectedValue("fatal: refusing to merge unrelated histories");
    await gitActions.squashMerge("feature");
    expect(errors.current).toMatchObject({
      title: "Squash merge failed",
      headline: "refusing to merge unrelated histories",
    });
  });

  it("reloadGraph: a repository that can't be opened", async () => {
    vi.spyOn(api, "loadGraph").mockRejectedValue(
      "Failed to spawn command: No such file or directory (os error 2)",
    );
    await reloadGraph();
    expect(errors.current).toMatchObject({
      title: "Couldn't open git-it-error-test",
      headline: "Failed to spawn command: No such file or directory (os error 2)",
    });
    expect(errors.current?.output).toContain(REPO);
  });
});

describe("runRemote reports only failures the user didn't choose", () => {
  it("reports a failed pull with git's real error", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ message: DIVERGENT }));
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Pull failed",
      headline: "Need to specify how to reconcile divergent branches.",
    });
    expect(appState.status).toBe("Pull failed: Need to specify how to reconcile divergent branches.");
  });

  it("reports a thrown error", async () => {
    vi.spyOn(api, "pull").mockRejectedValue("A remote operation is already in progress.");
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current?.headline).toBe("A remote operation is already in progress.");
  });

  it("leaves conflicts to the conflict panel", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ conflicted: true, message: "CONFLICT" }));
    await gitActions.pull();
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull: conflicts to resolve.");
  });

  it("treats a pull the user cancelled as cancelled", async () => {
    let finish: (o: RemoteOutcome) => void = () => {};
    vi.spyOn(api, "pull").mockImplementation(
      () => new Promise<RemoteOutcome>((resolve) => (finish = resolve)),
    );
    vi.spyOn(api, "cancelRemote").mockResolvedValue(undefined);
    const pulling = gitActions.pull();
    void gitActions.cancelRemote();
    finish(outcome({ message: "Cancelled." }));
    expect(await pulling).toBe(false);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull cancelled.");
  });

  it("does not report a dismissed sign-in", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(
      outcome({ authFailed: true, message: "fatal: Authentication failed" }),
    );
    vi.spyOn(dialogs, "confirmCredentials").mockResolvedValue(null);
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull: sign-in cancelled.");
  });

  it("reports a retry that fails after signing in", async () => {
    vi.spyOn(api, "pull")
      .mockResolvedValueOnce(outcome({ authFailed: true, message: "fatal: Authentication failed" }))
      .mockResolvedValueOnce(
        outcome({
          authFailed: true,
          message:
            "remote: Invalid username or token.\nfatal: Authentication failed for 'https://github.com/o/r.git/'",
        }),
      );
    vi.spyOn(dialogs, "confirmCredentials").mockResolvedValue({ username: "u", password: "p" });
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current?.headline).toBe(
      "Authentication failed for 'https://github.com/o/r.git/'",
    );
  });

  it("does not report a stale auth failure after the repository changed", async () => {
    const pull = vi
      .spyOn(api, "pull")
      .mockResolvedValue(outcome({ authFailed: true, message: "fatal: Authentication failed" }));
    vi.spyOn(dialogs, "confirmCredentials").mockImplementation(async () => {
      appState.repo = "/tmp/git-it-other-repo";
      return { username: "u", password: "p" };
    });
    expect(await gitActions.pull()).toBe(false);
    expect(pull).toHaveBeenCalledTimes(1);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Repository changed while that dialog was open — nothing was done.");
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/lib/gitActions.errorReporting.test.ts`
Expected: FAIL. Every "open the error popup" case sees `errors.current` as `null`, and the cancel and sign-in cases see the old status text.

- [ ] **Step 3: Import the report helpers**

In `src/lib/gitActions.ts`, below `import { prefillFromSubjects } from "./github/prCreate";`, add:

```ts
import { errorText } from "./errorReport";
import { reportError } from "./errors.svelte";
```

- [ ] **Step 4: Report a repository that can't be opened**

In `reloadGraph`, inside the `catch (e)` for `api.loadGraph`, replace:

```ts
          appState.status = `Could not open ${target}: ${e}`;
```

with:

```ts
          // The panels just went blank; say why. The full path goes in the output.
          const name = target.split("/").filter(Boolean).pop() ?? target;
          reportError(`Couldn't open ${name}`, `${target}\n${errorText(e)}`);
```

- [ ] **Step 5: Route every wrapper's failure through `reportError`**

In `run`, replace:

```ts
    appState.status = `${label} failed: ${firstLine(e)}`;
    // The status bar alone is too easy to miss for a discrete action the user just took
    // (user-reported "no feedback for if something went wrong") — surface it as a dialog.
    void dialogs.alert({ title: `${label} failed`, message: errorText(e) });
    return false;
```

with:

```ts
    reportError(`${label} failed`, e);
    return false;
```

Delete the now-orphaned helpers directly below `run`:

```ts
// Full error text. GitHub commands reject with a serialized GithubError OBJECT
// ({kind, message?}) — String() on that yields "[object Object]", so unwrap it first
// (mirrors errText in githubActions). Used verbatim in the error dialog.
function errorText(e: unknown): string {
  if (e && typeof e === "object" && "kind" in e) {
    const g = e as { kind: string; message?: string };
    return String(g.kind === "Other" ? (g.message ?? g.kind) : g.kind);
  }
  return String(e);
}
// First line only — git output is often multiline; the compact status bar gets one line.
function firstLine(e: unknown): string {
  return errorText(e).split("\n")[0];
}
```

In each of `runOp`, `runResolve`, `runWorktree`, `runDestructive` and `runDestructiveRebase`, replace the catch block's:

```ts
    appState.status = `${label} failed: ${firstLine(e)}`;
```

with:

```ts
    reportError(`${label} failed`, e);
```

- [ ] **Step 6: Rewrite `runRemote` with its reporting rules**

Replace the whole `runRemote` function, from its `// ── Remote operation helper (Phase 6)` comment header to its closing brace, with:

```ts
// Set by gitActions.cancelRemote, read by runRemote: a pull/push that ends after the user pressed
// Cancel was cancelled, not failed — whatever git printed on its way out.
let remoteCancelRequested = false;

// ── Remote operation helper (Phase 6) ────────────────────────────────────────
// Runs a streamed pull/push op with:
//   1. Progress state management (startRemoteProgress / pushRemoteLog / endRemoteProgress)
//   2. First attempt WITHOUT credentials (relies on system credential helper / SSH agent)
//   3. On authFailed: prompt for credentials and retry
//   4. Graph reload + status update (conflicted → ConflictView; ok → done)
//   5. The error popup, but only for failures the user didn't choose (not Cancel, not a
//      dismissed sign-in, not a repository switch during the prompt)
async function runRemote(
  label: string,
  fn: (onLine: (l: string) => void, creds?: { username: string; password: string }) => Promise<RemoteOutcome>,
): Promise<boolean> {
  if (appState.remoteOpActive) { appState.status = "A remote operation is already in progress."; return false; }
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return false;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return false;
  }
  // Capture before the FIRST attempt, not before the credentials prompt: a switch during
  // that first await would otherwise be recorded as the "original" repo and the guard would
  // wave the retry through.
  const sameRepo = sameRepoAfterPrompt();
  remoteCancelRequested = false;
  appState.startRemoteProgress(label);
  try {
    // First attempt: no credentials (system helper / SSH agent / keychain).
    let outcome = await fn((l) => appState.pushRemoteLog(l));
    let signInCancelled = false;
    let repoChanged = false;

    if (outcome.authFailed && !remoteCancelRequested) {
      // Auth failed — prompt for credentials and retry once. `fn` reads `appState.repo` when
      // invoked, so a switch anywhere in this flow would retry against the NEW repository with
      // the credentials just entered — pushing to a remote the user never chose.
      const creds = await dialogs.confirmCredentials({ title: `${label}: sign in` });
      if (!creds) signInCancelled = true;
      else if (!sameRepo()) repoChanged = true;
      else outcome = await fn((l) => appState.pushRemoteLog(l), creds);
    }

    // Refresh graph/status so ConflictView, UndoBar, ahead/behind etc. are current.
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] graph refresh after remote op failed", e);
    }

    if (outcome.conflicted) {
      appState.status = `${label}: conflicts to resolve.`;
    } else if (outcome.ok) {
      appState.status = `${label} — done.`;
    } else if (remoteCancelRequested) {
      appState.status = `${label} cancelled.`;
    } else if (signInCancelled) {
      appState.status = `${label}: sign-in cancelled.`;
    } else if (!repoChanged) {
      // (When the repository changed, sameRepo() has already said why nothing happened.)
      reportError(`${label} failed`, outcome.message);
    }

    return outcome.ok;
  } catch (e) {
    try {
      await reloadGraph();
    } catch {}
    if (remoteCancelRequested) appState.status = `${label} cancelled.`;
    else reportError(`${label} failed`, e);
    return false;
  } finally {
    remoteCancelRequested = false;
    appState.endRemoteProgress();
  }
}
```

- [ ] **Step 7: Report the direct paths, and mark Cancel**

In `gitActions.amendCommit`, replace:

```ts
      appState.status = `Amend commit failed: ${firstLine(e)}`;
```

with:

```ts
      reportError("Amend commit failed", e);
```

In `gitActions.squashMerge`, replace:

```ts
      appState.status = `Squash merge failed: ${firstLine(e)}`;
```

with:

```ts
      reportError("Squash merge failed", e);
```

Replace:

```ts
  cancelRemote: () => api.cancelRemote(),
```

with:

```ts
  cancelRemote: () => {
    remoteCancelRequested = true;
    return api.cancelRemote();
  },
```

- [ ] **Step 8: Keep the partial-failure tests from leaking reports**

In `src/lib/gitActions.partialFailure.test.ts`, add below `import { dialogs } from "./dialogs.svelte";`:

```ts
import { errors } from "./errors.svelte";
```

and in `afterEach`, directly below `dialogs.resolveAlert();`, add:

```ts
  errors.reset();
```

- [ ] **Step 9: Run the tests and the type gate**

Run: `npx vitest run src/lib/gitActions.errorReporting.test.ts src/lib/gitActions.partialFailure.test.ts`
Expected: PASS.

Run: `grep -n "firstLine" src/lib/gitActions.ts`
Expected: no output.

Run: `npm run check`
Expected: 0 errors and 0 warnings.

- [ ] **Step 10: Commit**

```bash
git add src/lib/gitActions.ts src/lib/gitActions.errorReporting.test.ts src/lib/gitActions.partialFailure.test.ts
git commit -F - <<'EOF'
fix: report every failed operation in the error popup

Only run() surfaced failures beyond one status line; pull, push, merge,
stash, staging, reset and rebase failures were easy to miss and showed
the wrong line of git's output. Route them all through reportError.
Pull and push stay quiet for a cancel, a dismissed sign-in, or a repo
switch during the sign-in prompt.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 6: Retire the alert dialog

**Files:**
- Modify: `src/lib/dialogs.svelte.ts` (delete the `alert` kind)
- Modify: `src/lib/components/Modal.svelte` (delete alert rendering, focus, keys and CSS)
- Modify: `src/lib/repositoryFlows.ts` (use `reportError`)
- Test: `src/lib/repositoryFlows.test.ts`, `src/lib/gitActions.partialFailure.test.ts`

**Interfaces:**
- Consumes: `reportError` (Task 2).
- Produces: `dialogs.alert` and `dialogs.resolveAlert` no longer exist.

- [ ] **Step 1: Update the repository-flow test to expect `reportError`**

In `src/lib/repositoryFlows.test.ts`:

Replace `  alert: vi.fn(),` in the `vi.hoisted` block with:

```ts
  reportError: vi.fn(),
```

Replace the dialogs mock:

```ts
vi.mock("./dialogs.svelte", () => ({
  dialogs: {
    newRepository: mocks.newRepository,
    confirm: mocks.confirm,
    alert: mocks.alert,
  },
}));
```

with:

```ts
vi.mock("./dialogs.svelte", () => ({
  dialogs: {
    newRepository: mocks.newRepository,
    confirm: mocks.confirm,
  },
}));
vi.mock("./errors.svelte", () => ({ reportError: mocks.reportError }));
```

In `beforeEach`, replace `  mocks.alert.mockReset().mockResolvedValue(undefined);` with:

```ts
  mocks.reportError.mockReset();
```

In "keeps and opens the local repository when GitHub setup fails", replace:

```ts
    expect(mocks.alert).toHaveBeenCalledWith(expect.objectContaining({
      title: "Local repository created",
      message: expect.stringContaining("name already exists"),
    }));
```

with:

```ts
    expect(mocks.reportError).toHaveBeenCalledWith(
      "Local repository created, but GitHub setup failed",
      "name already exists",
    );
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/repositoryFlows.test.ts`
Expected: FAIL, because `repositoryFlows.ts` still calls `dialogs.alert`, which the mock no longer provides ("dialogs.alert is not a function").

- [ ] **Step 3: Switch `repositoryFlows.ts` to `reportError`**

In `src/lib/repositoryFlows.ts`, add below `import { dialogs } from "./dialogs.svelte";`:

```ts
import { reportError } from "./errors.svelte";
```

Replace:

```ts
    } catch (error) {
      const message = githubErrorMessage(error);
      appState.status = `Local repository created; GitHub setup failed: ${message}`;
      await dialogs.alert({
        title: "Local repository created",
        message: `The local repository is ready at ${initialized.path}, but GitHub setup failed:\n\n${message}`,
      });
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    appState.status = `Create repository failed: ${message}`;
    await dialogs.alert({ title: "Create repository failed", message });
  } finally {
```

with:

```ts
    } catch (error) {
      reportError("Local repository created, but GitHub setup failed", githubErrorMessage(error));
    }
  } catch (error) {
    reportError("Create repository failed", error);
  } finally {
```

- [ ] **Step 4: Delete the alert dialog kind**

In `src/lib/dialogs.svelte.ts`, delete this member of the `DialogState` union:

```ts
  | {
      // A message-only error dialog: single "OK" button, nothing to confirm. Surfaces a
      // failed op unmissably instead of only in the easy-to-miss status bar.
      kind: "alert";
      title: string;
      message: string;
      resolve: () => void;
    }
```

In `settlePending()`, delete:

```ts
    else if (state.kind === "alert") state.resolve();
```

Delete the alert methods:

```ts
    // ── Alert dialog ──────────────────────────────────────────────────────────
    // Message-only "something failed" modal. The returned promise resolves when the
    // dialog is dismissed; callers can fire-and-forget (nothing waits on it).
    alert(opts: { title: string; message: string }): Promise<void> {
      settlePending();
      return new Promise((resolve) => {
        state = { kind: "alert", title: opts.title, message: opts.message, resolve };
      });
    },
    resolveAlert() {
      if (state.kind === "alert") {
        state.resolve();
        state = { kind: "none" };
      }
    },
```

In `src/lib/components/Modal.svelte`:
- Delete `  let okAlertEl = $state<HTMLButtonElement | undefined>();`.
- In the focus `$effect`, delete:
  ```ts
      } else if (s.kind === "alert") {
        okAlertEl?.focus();
  ```
- Delete the `handleAlertKey` function:
  ```ts
    function handleAlertKey(e: KeyboardEvent) {
      if (e.key === "Enter" || e.key === "Escape") {
        e.preventDefault();
        dialogs.resolveAlert();
      }
    }
  ```
- Delete the whole `{:else if dialogs.state.kind === "alert"}` branch, keeping the final `{/if}` that closes the chain:
  ```svelte
  {:else if dialogs.state.kind === "alert"}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="overlay"
      onpointerdown={(e) => {
        if (e.target === e.currentTarget) dialogs.resolveAlert();
      }}
      onkeydown={handleAlertKey}
    >
      <div class="dialog" role="alertdialog" aria-modal="true" aria-label={dialogs.state.title}>
        <h3>{dialogs.state.title}</h3>
        <p class="msg pre">{dialogs.state.message}</p>
        <div class="actions">
          <button type="button" class="primary" bind:this={okAlertEl} onclick={() => dialogs.resolveAlert()}>OK</button>
        </div>
      </div>
    </div>
  ```
- Delete the alert-only CSS rule:
  ```css
    /* Raw git/tool error text: preserve line breaks, wrap long paths, and cap height so a
       verbose stderr scrolls instead of growing the dialog off-screen. */
    .msg.pre {
      white-space: pre-wrap;
      word-break: break-word;
      max-height: 240px;
      overflow-y: auto;
    }
  ```

In `src/lib/gitActions.partialFailure.test.ts`, delete `  dialogs.resolveAlert();` from `afterEach` and the now-unused `import { dialogs } from "./dialogs.svelte";`.

- [ ] **Step 5: Verify nothing else uses the alert**

Run: `grep -rn "dialogs.alert\|resolveAlert\|kind === \"alert\"\|okAlertEl\|handleAlertKey" src`
Expected: no output.

- [ ] **Step 6: Run the tests and the type gate**

Run: `npx vitest run src/lib/repositoryFlows.test.ts src/lib/gitActions.partialFailure.test.ts`
Expected: PASS.

Run: `npm run check`
Expected: 0 errors and 0 warnings.

- [ ] **Step 7: Commit**

```bash
git add src/lib/dialogs.svelte.ts src/lib/components/Modal.svelte src/lib/repositoryFlows.ts src/lib/repositoryFlows.test.ts src/lib/gitActions.partialFailure.test.ts
git commit -F - <<'EOF'
refactor: replace the alert dialog with error reports

Its last callers now use reportError. The alert shared the single dialog
slot, so opening it cancelled whatever prompt was on screen; the error
popup has its own layer.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 7: Gate, real-app verification, PR

**Files:** none changed, unless verification finds a defect. In that case fix it in the owning task's files with a new commit.

- [ ] **Step 1: Run the full gate**

```bash
npm run check
npm test
cargo test
cargo clippy -p git-core -p git-it -- -D warnings
```

Expected:
- `npm run check`: 0 errors and 0 warnings.
- `npm test`: every file passes, including the 5 new ones.
- `cargo test` and clippy: clean. This PR changes no Rust.

- [ ] **Step 2: Build the debug app**

`tauri dev` is invisible to screenshots, so build a bundled debug app. Don't run this while `tauri dev` is running.

```bash
npm run tauri build -- --debug --bundles app
```

Expected: `target/debug/bundle/macos/Git It.app` exists.

- [ ] **Step 3: Build throwaway repositories**

```bash
V="${TMPDIR%/}/gitit-verify"; rm -rf "$V"; mkdir -p "$V" && cd "$V"
git init -q --bare origin.git
git clone -q origin.git seed && cd seed
printf 'one\ntwo\nthree\n' > notes.txt && git add notes.txt && git commit -qm "base" && git push -q origin HEAD
cd "$V" && git clone -q origin.git diverged && git clone -q origin.git behind
cd "$V/seed" && sed -i '' 's/one/ONE/' notes.txt && git commit -qam "remote change" && git push -q
cd "$V/diverged" && git commit -q --allow-empty -m "local change"
cd "$V/behind" && sed -i '' 's/one/uno/' notes.txt
echo "$V"
```

What each repo is for:
- **`diverged`:** Pull fails with the divergent-branches error; Push is rejected.
- **`behind`:** Pull fails with "Your local changes … would be overwritten".

- [ ] **Step 4: Swap in the debug app**

```bash
osascript -e 'tell application "Git It" to quit'
open -n "target/debug/bundle/macos/Git It.app"
```

With computer-use, `request_access` for **both** "Git It" and "Finder". Vibrancy makes clicks hit-test through to the desktop without the Finder grant. If the update-check modal appears, press **Cancel**, never Download. Open `diverged` and `behind` from the throwaway folder.

- [ ] **Step 5: Drive the scenarios and screenshot each**

1. In `diverged`, click **Pull**. Expected:
   - Popup titled "Pull failed", headline "Need to specify how to reconcile divergent branches."
   - A hints block containing `git config pull.rebase false  # merge`.
   - Status bar red: "Pull failed: Need to specify how to reconcile divergent branches."
2. Click **Show git output**. Expected: the raw output, starting with `From …origin`.
3. Click **Copy details**. Expected: the button reads "Copied", and `read_clipboard` returns text starting `Pull failed\nNeed to specify…`.
4. Click **OK**, then click the red status text. Expected: the same popup reopens.
5. In `diverged`, click **Push**. Expected: "Push failed", headline "failed to push some refs to '…origin.git'", and the "Updates were rejected…" hints.
6. In `behind`, click **Pull**. Expected: "Pull failed", headline "Your local changes to the following files would be overwritten by merge:" followed by `notes.txt` and "Please commit your changes or stash them before you merge."
7. Settings → Appearance → NERV, then repeat scenario 1. Expected: the popup has NERV brackets and the ◇ title prefix, and stays legible.
8. **Escape:** synthetic Escape never reaches WKWebView (a tooling limit). Ask the user to press Escape once on an open popup and confirm it closes.

- [ ] **Step 6: Restore the installed app**

```bash
osascript -e 'tell application "Git It" to quit'
open -a "/Applications/Git It.app"
```

- [ ] **Step 7: Push and open the PR**

```bash
git -c credential.helper= -c credential.helper='!gh auth git-credential' push -u origin feat/error-reporting-core
gh pr create --base next --head feat/error-reporting-core \
  --title "feat: show failed operations in an error popup" \
  --body-file - <<'EOF'
## Summary
- Failed operations now open a popup with git's actual error, its hints, and the raw output behind a disclosure (Copy details, OK). Before this, the status bar showed the *first* line of git's output, e.g. `From github.com:…` for a failed pull.
- Every gitActions wrapper reports through one entry point (`reportError`): pull, push, merge, cherry-pick, revert, conflict resolution, staging, stash, commit, reset, amend, rebase, squash merge. Failing to open a repository reports too.
- The status bar turns red on failure and reopens the error when clicked.
- Errors queue in their own layer, so a failure never cancels an open prompt. The old `dialogs.alert`, which shared the dialog slot, is removed.
- Pull and push stay quiet for a user Cancel, a dismissed sign-in, or a repository switch during sign-in.

Spec: `docs/superpowers/specs/2026-09-26-error-reporting-and-pull-dialog-design.md` (Part A). Parts B (coverage) and C (Pull dialog) follow in separate PRs.

## Test plan
- [x] `npm run check` (0 errors, 0 warnings)
- [x] `npm test`, including new errorReport / errors / ErrorDialog / StatusBar / gitActions.errorReporting suites
- [x] `cargo test`, `cargo clippy -p git-core -p git-it -- -D warnings`
- [x] Debug `.app` against throwaway repos: divergent pull, rejected push, overwritten local changes, Show output, Copy details, reopen from the status bar, NERV theme

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
```

Then bind the PR with the ccd_pr tools and follow the Codex review loop:
- fix findings in this PR
- pushing triggers the next round
- a `+1` on the PR body is the clean signal
- resolve each thread with GraphQL `resolveReviewThread` after fixing it
