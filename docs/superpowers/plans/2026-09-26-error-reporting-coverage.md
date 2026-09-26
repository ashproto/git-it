# Error Reporting Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cover Part B (B1–B5) of the approved error-reporting spec with visible action failures, refusal notices, retryable inline load errors, and quiet background reports.

**Architecture:** Consume PR 1's error queue and pure summarizer without changing their contracts. Keep reporting at the owner of each action, render load failures through one small Svelte component, and preserve existing cancellation, refresh, and browser-preview behavior.

**Tech Stack:** Tauri 2, Rust, SvelteKit with Svelte 5 runes, TypeScript, Vitest, jsdom.

## Global Constraints

- Gate before claiming done: `npm run check` (0 errors, 0 warnings) + `npm test` + `cargo test` + `cargo clippy -p git-core -p git-it -- -D warnings` (CI's exact clippy command).
- Surgical changes only; match surrounding style/comment density; remove only what your change orphans.
- Commits: conventional commits, subject lowercase-first, ≤100 chars (commitlint). Every commit message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Use a heredoc for multi-line messages.
- Svelte 5 runes; never `:global()` in plain `nerv.css`.
- The body defaults to `user-select: none`; add class `selectable` to error text users may want to copy.
- Execute on `feat/error-reporting-coverage`, branched from `next` after PR 1 merges. Target `next`; no stacked PR and no stable promotion here.
- Implement Part B only. Do not change the entire `gitActions.pull` export, including its loading guard; PR 3 owns it. Do not implement the error queue, dialog, status-bar UI, parser, or backend pull strategy.
- A wrapper returning `false` owns its report. Callers must not report it again. Refusals use `reportNotice`; browser-preview guards remain status-only.
- Keep the UI's existing messages verbatim as notice bodies. Errors use PR 1's summary, preserving raw details in its Output log.
- All commands below run from the repository root. Test commands and commits are instructions for future implementation, not evidence that this planning document has executed them.
- Every PR goes through adversarial review and the Codex review loop until approval, with review threads resolved. Nothing reaches stable until a separate `next → main` promotion.

---

## Ambiguities resolved

- Update checks already have a `source: "manual" | "startup" | "background" | "activation"` discriminator. Reuse it; add no manual parameter. Download/install are user-approved actions even when an automatic check discovered the update, so their failures open popups.
- Keep Open With application-discovery failure as silent submenu omission. Report only the user's chosen Open, Open With, or Show in Finder action.
- For `runResolve`, no desktop gets the existing desktop-only status text; desktop with no repository gets a notice. Keep squash merge's existing combined desktop/repository guard status-only as explicitly excluded; only its busy refusal gains a notice.
- Inline loads keep the current list on screen while a refresh runs — the app's existing rule for repository switches, and the stash list refreshes on every watcher tick. Only the first load shows pending copy; a failure replaces the list with the error and Retry; superseded responses are dropped. `loaded` flips to true only on success and a loader never resets it (Backups resets it on repository switch). An error is retained even when the rejection is null/undefined. A real zero-item success alone gets empty-state copy. Retry reuses the same loader.
- Preserve ApplyPanel's streamed and lifecycle logging. Change its redundant raw-error append to `[rewrite] stopped.`; `reportError` logs the raw failure once.
- Keep the updater's signature-verification explanation and underlying error. The backend combines download and installation, so use "Couldn't download or install the update" for that command and "Couldn't restart to install the update" for relaunch. A watchdog report suppresses a second report if that same relaunch later rejects.
- Missing clipboard support is an action failure with `Clipboard is unavailable.`; it must not look like a successful copy.
- The conflict panel's resolution and Continue/Amend controls stay driven by git's own conflict count (authoritative, from a call that succeeded); a failed details load only replaces the unexplained empty list with an inline error and Retry.
- Notice titles derived from an operation label lowercase only the label's first character (`cantTitle`: "Can't checkout Feature/ABC"). The rest of the label carries ref and remote names exactly as the user typed them.
- Titles read as natural English with articles: "Couldn't open the repository", "Couldn't watch the repository", "Couldn't create the bundle". `LoadError` renders `Couldn't load {what}`, so every `what` is phrased to follow it: "the stashes", "the reflog", "the conflict details", "the diff", "the bundles", "the safety refs", "the GitHub screen".
- Component tests that mount `WorkingCopyView` stub `window.matchMedia`: `DiffView` reads it at init and jsdom has none.

## Grounding and PR boundaries

Read in full: `CLAUDE.md`, `docs/superpowers/specs/2026-09-26-error-reporting-and-pull-dialog-design.md`, and `/Users/ashshah/.claude/plugins/cache/claude-plugins-official/superpowers/6.1.1/skills/writing-plans/SKILL.md`.

Source inventory is reverified against `feat/error-reporting-core` at `340ee99530495ff4b60d56552c8195fbc616fe53`; the quoted source is unchanged from the initial read at `209fc16646698f36e1f978fca0c009e88ed109bb`. Locations below are audit evidence only; locate edits by the named function/component and exact quoted code, since PR 1 shifts line numbers. Every **Before** quotation comes from that current source, except two that quote PR 1's result because PR 1 rewrites them first: `src/lib/repositoryFlows.test.ts` (Task 1) and `gitActions.cancelRemote` (Task 13). New files and **After** blocks are proposed code.

PR 1 is an execution prerequisite, not work in this plan. It supplies these exact interfaces:

```ts
// src/lib/errorReport.ts
export type GitErrorSummary = { headline: string; hints: string; output: string };
export function errorText(e: unknown): string;
export function summarizeGitOutput(text: string): GitErrorSummary;

// src/lib/errors.svelte.ts
export type ErrorReport = {
  id: number;
  kind: "error" | "notice";
  title: string;
  headline: string;
  hints: string;
  output: string;
};
export const errors: {
  readonly current: ErrorReport | null;
  readonly pending: number;
  readonly last: ErrorReport | null;
  dismiss(): void;
  reopenLast(): void;
  reset(): void;
};
export function reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void;
export function reportNotice(title: string, message: string): void;
// appState additions: statusIsError: boolean; setErrorStatus(text: string): void
// appState.status = text clears statusIsError.
```

PR 1 already imports/uses `reportError` in `gitActions.ts` and `repositoryFlows.ts`, reports wrapper failures, removes `dialogs.alert`/`resolveAlert`, and introduces `remoteCancelRequested`. Where an import is extended below, merge with that existing import; do not duplicate it. Do not restore the old wrapper catch bodies or alert API from the current-tree quotations. PR 1 also rewrites `src/lib/repositoryFlows.test.ts` to mock `./errors.svelte` with `reportError` only; Task 1 extends that mock with `reportNotice`, or its overlap test throws `No "reportNotice" export is defined on the "./errors.svelte" mock`. New tests use the real PR 1 reporting store and reset it after each test; never spy on ESM reporting functions.

| Verified site | Current source location and correction |
|---|---|
| Open picker and validation | `src/lib/repositoryFlows.ts` `openRepositoryFlow`, lines 25–37; creation overlap at 44–47 |
| Recent repository validation | `src/lib/components/RepoList.svelte` 26–36; `src/lib/components/RepoTabs.svelte` 202–213 |
| Native file actions / diff | `src/lib/components/WorkingCopyView.svelte` 234, 245, 251 / 127–151; rendering is 589–594 |
| Worktree folder | `src/lib/components/WorktreePanel.svelte` 80 |
| Clipboard | `src/lib/components/GraphHistory.svelte` 326; **full path** `src/lib/components/github/PrCommitsTab.svelte` 26–35 |
| Reword prechecks/refusals | `src/lib/gitActions.ts` `setCommitMessage` 876–914 |
| Message loads | `src/lib/components/CommitComposer.svelte` 75–106; `src/lib/components/CommitMessageEdit.svelte` 45–59 |
| Rewrite / date validations | `src/lib/components/ApplyPanel.svelte` 27–34, 71–74; `src/lib/components/EditTabs.svelte` 64, 96, 107, 113, 117, 133 |
| Backup lists/actions | `src/lib/components/BackupsPanel.svelte` 20–39, 41–105 |
| Stashes / reflog / conflicts | `src/lib/components/StashPanel.svelte` 27–35; `src/lib/components/ReflogPanel.svelte` 32–40; `src/lib/components/ConflictView.svelte` 18–37 |
| Existing inline diff look | `src/lib/components/CommitDetail.svelte` 170–183, `.note` and `.err` styles; use compact note typography but the specified `--danger` token |
| GitHub availability | `src/lib/githubState.svelte.ts` 194–231; `src/lib/components/github/GithubSetupCard.svelte` 1–21; caller is **GithubView.svelte**, not `GitHubView.svelte` |
| Watcher / cancellation | `src/lib/gitActions.ts` `startWatchingRepo` 244–251 / `cancelRemote` 1197 (PR 1 changes cancellation) |
| Common operation guards | `src/lib/gitActions.ts` 419–707; direct guards 947, 980, 1018, 1070, 1091; undo 1115; squash busy 1136; PR guards 1237, 1245 |
| Loading guards | `src/lib/gitActions.ts` fetch 815–818 and push 1166–1169; pull 1152–1161 is excluded |
| Sidebar / rebase validation | `src/lib/components/Sidebar.svelte` 80–101; `src/lib/components/RebaseTodo.svelte` 81–86 |
| Updater | `src/lib/updater.svelte.ts` 105–145, 182–254; Settings caller 417, macOS menu caller `src/routes/+page.svelte` 279; no caller edit needed |

The approximate site hints match these bodies. Corrections/clarifications are the full GitHub component paths, the case-sensitive `GithubView.svelte` caller, the additional diff rendering location, and the actual function boundaries above; no hinted site is omitted.

## Not changing (B5 and adjacent boundaries)

Settings-persistence failures stay in the console, including Tauri-store writes. GitHub list panels and GitHub write actions keep their existing inline errors and open write modals. Shiki highlight fallback, `tauriDrag`, localStorage reads, graph paging failures, and jump-to-ref's existing message remain unchanged. GitHub `softRefresh` retains its current transient-failure behavior. `CommitDetail`'s existing "Could not load diff:" copy stays as it is. The prerequisite tooltip, loss of typed input after message update/amend/rebase failures, and Pull strategy/dialog/settings/backend work remain out of scope.

## File Structure

| Task | Production files modified / created | New complete test files |
|---|---|---|
| 1 | `src/lib/repositoryFlows.ts`; `src/lib/components/RepoList.svelte`; `src/lib/components/RepoTabs.svelte`; existing `src/lib/repositoryFlows.test.ts` (its `./errors.svelte` mock gains `reportNotice`) | `src/lib/repositoryFlows.coverage.test.ts`; `src/lib/components/RepoRecents.coverage.test.ts` |
| 2 | `src/lib/gitActions.ts` guards only | `src/lib/gitActions.notices.test.ts` |
| 3 | `src/lib/gitActions.ts` `setCommitMessage` only | `src/lib/gitActions.reword.test.ts` |
| 4 | `src/lib/components/CommitComposer.svelte`; `src/lib/components/CommitMessageEdit.svelte` | `src/lib/components/CommitMessage.coverage.test.ts` |
| 5 | `src/lib/components/WorkingCopyView.svelte`; `src/lib/components/WorktreePanel.svelte`; `src/lib/components/GraphHistory.svelte`; `src/lib/components/github/PrCommitsTab.svelte` | `src/lib/components/NativeActions.coverage.test.ts` |
| 6 | `src/lib/components/ApplyPanel.svelte`; `src/lib/components/EditTabs.svelte`; `src/lib/components/RebaseTodo.svelte` | `src/lib/components/EditFailures.coverage.test.ts` |
| 7 | Create `src/lib/components/LoadError.svelte`; modify `src/lib/components/StashPanel.svelte`; `src/lib/components/ReflogPanel.svelte` | `src/lib/components/LoadError.test.ts`; `src/lib/components/HistoryLoads.coverage.test.ts` |
| 8 | `src/lib/components/ConflictView.svelte` | `src/lib/components/ConflictView.coverage.test.ts` |
| 9 | `src/lib/components/WorkingCopyView.svelte` diff loader, its diff-pane branch and one style rule | `src/lib/components/WorkingCopyView.load.test.ts` |
| 10 | `src/lib/components/BackupsPanel.svelte` | `src/lib/components/BackupsPanel.coverage.test.ts` |
| 11 | `src/lib/githubState.svelte.ts`; `src/lib/components/github/GithubSetupCard.svelte`; `src/lib/components/github/GithubView.svelte` | `src/lib/components/github/GithubAvailability.coverage.test.ts` |
| 12 | `src/lib/updater.svelte.ts` | `src/lib/updater.coverage.test.ts` |
| 13 | `src/lib/gitActions.ts` cancellation and watcher only | `src/lib/gitActions.background.test.ts` |
| 14 | `src/lib/components/Sidebar.svelte` and final gates | `src/lib/components/Sidebar.notices.test.ts` |

No Rust or configuration edits are planned. Existing tests remain in place, except that Task 1 extends the `./errors.svelte` mock in PR 1's `src/lib/repositoryFlows.test.ts`.

## Refusal copy table (B2)

| Site | Notice title | Message body |
|---|---|---|
| `run`, `runOp`, `runResolve`, `runWorktree`, `runDestructive`, `runDestructiveRebase`, `runRemote` | `cantTitle(label)` — "Can't " + the label with only its first character lowercased, e.g. "Can't checkout Feature/ABC" | Open a repository first. |
| `discardHunk`, `discardLines`, `amendCommit`, `discard`, `clean` | Can't discard the hunk / Can't discard the lines / Can't amend the commit / Can't discard changes / Can't remove untracked files | Open a repository first. |
| `setCommitMessage` desktop/no repository precheck | Can't reword this commit | Open a repository first. |
| `fetch` / `push` | Can't fetch yet / Can't push yet | Repository is still loading — try again in a moment. |
| `runRemote` | `${cantTitle(label)} yet` | A remote operation is already in progress. |
| `sameRepoAfterPrompt` in gitActions and Sidebar | Repository changed | Repository changed while that dialog was open — nothing was done. |
| Sidebar `refActionsBlocked` | Repository still opening | Still opening the repository — try that again in a moment. |
| `setCommitMessage` root | Can't reword this commit | Can't reword the root commit here. |
| `setCommitMessage` merge | Can't reword this commit | Can't reword a merge commit here. |
| `setCommitMessage` range merge | Can't reword this commit | Can't reword across a merge commit yet. |
| `setCommitMessage` missing target | Can't reword this commit | Can't reword that commit — it isn't in the current branch's history. |
| `undo` | Can't undo | Undo unavailable — the checked-out branch changed. |
| `createPullRequest` detached | Can't create a pull request | No branch checked out — check out a branch to create a pull request. |
| `createPullRequest` no base | Can't create a pull request | No other local branch to use as the PR base. |
| `openRepositoryFlow`, `RepoList.openRecentRepo`, `RepoTabs.openRecent` | Not a git repository | `${path} is not a git repo.` |
| `createRepositoryFlow` overlap | Can't create a repository yet | A repository is already being created. |
| ApplyPanel prerequisites | Can't rewrite history | Commit-time editing is unavailable — the bundled git-filter-repo couldn't run. |
| ApplyPanel no dates | Can't rewrite history | No new dates queued. Use an edit mode to preview changes first. |
| EditTabs `needsSelection` | Can't preview date changes | Select one or more commits first. |
| EditTabs `previewExact` and `previewCompress` invalid date | Can't preview date changes | Enter a complete, valid date. |
| EditTabs `previewCompress` selection count | Can't preview date changes | Compress needs at least 2 selected commits. |
| EditTabs `previewCompress` ordering | Can't preview date changes | Compress: end must be after start. |
| EditTabs `previewCompress` unreadable dates | Can't preview date changes | Compress: couldn't read dates for the selected commits. |
| RebaseTodo `startRebase` | Can't start the interactive rebase | Interactive rebase: at least one non-drop step is required. |
| updater manual overlap only | Can't check for updates yet | Already checking for updates… |
| `squashMerge` remote/rewrite busy | Can't squash merge yet | Wait for the current remote operation or history rewrite to finish. |

All desktop-preview messages listed in the brief stay ordinary status writes. Pull's loading guard belongs to PR 3.

### Task 1: Report repository-opening failures and refusals

**Files:**
- Modify: `src/lib/repositoryFlows.ts` (`openRepositoryFlow`, creation overlap guard).
- Modify: `src/lib/components/RepoList.svelte` (`openRecentRepo`).
- Modify: `src/lib/components/RepoTabs.svelte` (`openRecent`).
- Modify: `src/lib/repositoryFlows.test.ts` (PR 1's `./errors.svelte` mock gains `reportNotice`).
- Create/Test: `src/lib/repositoryFlows.coverage.test.ts`.
- Create/Test: `src/lib/components/RepoRecents.coverage.test.ts`.

**Interfaces:**
- Consumes: `pickRepoFolder(initial?: string, title?: string): Promise<string | null>`; `api.isGitRepo(repo: string): Promise<boolean>`; `appState.openRepo(path: string): void`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`; `reportNotice(title: string, message: string): void`.
- Produces: unchanged `openRepositoryFlow(initial?: string): Promise<void>`, `createRepositoryFlow(initial?: string): Promise<void>`; component-local `openRecentRepo(path: string): Promise<void>` and `openRecent(path: string): Promise<void>` now consume their own rejected promises.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/repositoryFlows.coverage.test.ts` with this complete content:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { appState } from "./store.svelte";
import { errors } from "./errors.svelte";

const mocks = vi.hoisted(() => ({ pick: vi.fn(), isGitRepo: vi.fn() }));
vi.mock("./api", async (original) => ({
  ...await original<typeof import("./api")>(),
  pickRepoFolder: mocks.pick,
  api: { ...((await original<typeof import("./api")>()).api), isGitRepo: mocks.isGitRepo },
}));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
import { createRepositoryFlow, openRepositoryFlow } from "./repositoryFlows";

beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.pick.mockReset().mockResolvedValue("/tmp/recent");
  mocks.isGitRepo.mockReset().mockResolvedValue(true);
  appState.repo = "";
  appState.status = "Ready";
});
afterEach(() => {
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

it.each(["picker", "validation"])("reports a rejected %s once", async (site) => {
  const open = vi.spyOn(appState, "openRepo").mockImplementation(() => {});
  (site === "picker" ? mocks.pick : mocks.isGitRepo).mockRejectedValue(new Error("fatal: access denied"));
  await expect(openRepositoryFlow()).resolves.toBeUndefined();
  expect(open).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't open the repository", headline: "access denied" });
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
});

it("notices an ordinary non-repository result", async () => {
  mocks.isGitRepo.mockResolvedValue(false);
  await openRepositoryFlow();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Not a git repository", headline: "/tmp/recent is not a git repo." });
  expect(appState.status).toBe("/tmp/recent is not a git repo.");
  expect(appState.statusIsError).toBe(false);
});

it("treats picker cancellation as a no-op and still opens valid repositories", async () => {
  const open = vi.spyOn(appState, "openRepo").mockImplementation(() => {});
  mocks.pick.mockResolvedValueOnce(null);
  await openRepositoryFlow();
  expect(errors.pending).toBe(0);
  expect(open).not.toHaveBeenCalled();
  await openRepositoryFlow();
  expect(open).toHaveBeenCalledWith("/tmp/recent");
});

it("notices overlapping creation without opening another picker", async () => {
  let finish!: (value: string | null) => void;
  mocks.pick.mockImplementationOnce(() => new Promise<string | null>((resolve) => { finish = resolve; }));
  const first = createRepositoryFlow();
  try {
    await createRepositoryFlow();
    expect(mocks.pick).toHaveBeenCalledOnce();
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't create a repository yet", headline: "A repository is already being created." });
  } finally {
    finish(null);
    await first;
  }
});

it("keeps desktop-preview guards status-only", async () => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  await openRepositoryFlow();
  expect(appState.status).toBe("Opening a repo needs the desktop app.");
  await createRepositoryFlow();
  expect(appState.status).toBe("Creating a repo needs the desktop app.");
  expect(errors.pending).toBe(0);
});
```

Create `src/lib/components/RepoRecents.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import RepoList from "./RepoList.svelte";
import RepoTabs from "./RepoTabs.svelte";

vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let dispose: (() => Promise<void>) | undefined;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.openRepo("/tmp/recent");
  appState.closeRepo("/tmp/recent");
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.forgetRepo("/tmp/recent");
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

it.each([RepoList, RepoTabs])("reports rejected recent validation and notices non-repositories", async (Component) => {
  const probe = vi.spyOn(api, "isGitRepo").mockRejectedValueOnce(new Error("fatal: permission denied")).mockResolvedValue(false);
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(Component, { target });
  dispose = () => unmount(panel);
  await tick();
  for (const kind of ["error", "notice"]) {
    target.querySelector<HTMLButtonElement>(".add-btn")?.click();
    await tick();
    const recent = target.querySelector<HTMLButtonElement>('button[title="/tmp/recent"]');
    expect(recent).not.toBeNull();
    recent!.click();
    await vi.waitFor(() => expect(errors.current?.kind).toBe(kind));
    expect(errors.pending).toBe(1);
    expect(errors.current?.title).toBe(kind === "error" ? "Couldn't open the repository" : "Not a git repository");
    expect(appState.statusIsError).toBe(kind === "error");
    errors.reset();
  }
  expect(probe).toHaveBeenCalledTimes(2);
  expect(appState.openRepos).not.toContain("/tmp/recent");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/repositoryFlows.coverage.test.ts src/lib/components/RepoRecents.coverage.test.ts`

Expected: FAIL because rejected open/validation promises escape, non-repository/overlap guards do not queue notices, and the recents click may be reported by Vitest as an unhandled rejection. On this PR's starting point the PR 1 imports must already resolve. `src/lib/repositoryFlows.test.ts` still passes at this point; it turns red only once Step 3 makes the overlap guard call `reportNotice`, which is why Step 3 also updates its mock.

- [ ] **Step 3: Implement repository reporting**

In `src/lib/repositoryFlows.ts`, extend PR 1's existing reporting import to exactly:

```ts
import { reportError, reportNotice } from "./errors.svelte";
```

This import is a PR 1 overlap. Its creation-error catches already use `reportError`; leave them intact.

`src/lib/repositoryFlows.ts` — replace exactly:

**Before**
```ts
export async function openRepositoryFlow(initial?: string): Promise<void> {
  if (!isTauri()) {
    appState.status = "Opening a repo needs the desktop app.";
    return;
  }
  const path = await pickRepoFolder(initial);
  if (!path) return;
  if (!(await api.isGitRepo(path))) {
    appState.status = `${path} is not a git repo.`;
    return;
  }
  appState.openRepo(path);
}
```

**After**
```ts
export async function openRepositoryFlow(initial?: string): Promise<void> {
  if (!isTauri()) {
    appState.status = "Opening a repo needs the desktop app.";
    return;
  }
  try {
    const path = await pickRepoFolder(initial);
    if (!path) return;
    if (!(await api.isGitRepo(path))) {
      reportNotice("Not a git repository", `${path} is not a git repo.`);
      return;
    }
    appState.openRepo(path);
  } catch (e) {
    reportError("Couldn't open the repository", e);
  }
}
```

`src/lib/repositoryFlows.ts` — replace exactly:

**Before**
```ts
    appState.status = "A repository is already being created.";
```

**After**
```ts
    reportNotice("Can't create a repository yet", "A repository is already being created.");
```

`src/lib/components/RepoList.svelte` — replace exactly:

**Before**
```ts
  import { api } from "../api";
```

**After**
```ts
  import { api } from "../api";
  import { reportError, reportNotice } from "../errors.svelte";
```

`src/lib/components/RepoList.svelte` — replace exactly:

**Before**
```ts
  async function openRecentRepo(path: string) {
    if (!isTauri()) {
      appState.status = "Opening a repo needs the desktop app.";
      return;
    }
    if (!(await api.isGitRepo(path))) {
      appState.status = `${path} is not a git repo.`;
      return;
    }
    appState.openRepo(path);
  }
```

**After**
```ts
  async function openRecentRepo(path: string) {
    if (!isTauri()) {
      appState.status = "Opening a repo needs the desktop app.";
      return;
    }
    try {
      if (!(await api.isGitRepo(path))) {
        reportNotice("Not a git repository", `${path} is not a git repo.`);
        return;
      }
      appState.openRepo(path);
    } catch (e) {
      reportError("Couldn't open the repository", e);
    }
  }
```

`src/lib/components/RepoTabs.svelte` — replace exactly:

**Before**
```ts
  import { api } from "../api";
```

**After**
```ts
  import { api } from "../api";
  import { reportError, reportNotice } from "../errors.svelte";
```

`src/lib/components/RepoTabs.svelte` — replace exactly:

**Before**
```ts
  async function openRecent(path: string) {
    closeDropdown();
    if (!isTauri()) {
      appState.status = "Opening a repo needs the desktop app.";
      return;
    }
    if (!(await api.isGitRepo(path))) {
      appState.status = `${path} is not a git repo.`;
      return;
    }
    appState.openRepo(path);
  }
```

**After**
```ts
  async function openRecent(path: string) {
    closeDropdown();
    if (!isTauri()) {
      appState.status = "Opening a repo needs the desktop app.";
      return;
    }
    try {
      if (!(await api.isGitRepo(path))) {
        reportNotice("Not a git repository", `${path} is not a git repo.`);
        return;
      }
      appState.openRepo(path);
    } catch (e) {
      reportError("Couldn't open the repository", e);
    }
  }
```

`src/lib/repositoryFlows.test.ts` (PR 1 overlap: these four quotations are PR 1's Task 6 result, not the current tree). Its `./errors.svelte` mock exports only `reportError`, so the overlap guard's new `reportNotice` call would throw inside the existing "guards the entire flow against overlapping creation entry points" test. Replace exactly:

**Before**
```ts
  reportError: vi.fn(),
```

**After**
```ts
  reportError: vi.fn(),
  reportNotice: vi.fn(),
```

**Before**
```ts
vi.mock("./errors.svelte", () => ({ reportError: mocks.reportError }));
```

**After**
```ts
vi.mock("./errors.svelte", () => ({
  reportError: mocks.reportError,
  reportNotice: mocks.reportNotice,
}));
```

**Before**
```ts
  mocks.reportError.mockReset();
```

**After**
```ts
  mocks.reportError.mockReset();
  mocks.reportNotice.mockReset();
```

**Before**
```ts
    expect(mocks.appState.status).toBe("A repository is already being created.");
```

**After**
```ts
    expect(mocks.reportNotice).toHaveBeenCalledWith(
      "Can't create a repository yet",
      "A repository is already being created.",
    );
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/repositoryFlows.coverage.test.ts src/lib/components/RepoRecents.coverage.test.ts src/lib/repositoryFlows.test.ts`

Expected: PASS; each failed open has one error, refusal bodies are unchanged, cancellation opens no popup, and the existing overlap test now asserts the notice through its mock.

- [ ] **Step 5: Commit**

```bash
git add src/lib/repositoryFlows.ts src/lib/components/RepoList.svelte src/lib/components/RepoTabs.svelte src/lib/repositoryFlows.test.ts src/lib/repositoryFlows.coverage.test.ts src/lib/components/RepoRecents.coverage.test.ts
git commit -F - <<'COMMIT'
fix: report repository opening failures and refusals

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 2: Turn operation guards into notices

**Files:**
- Modify: `src/lib/gitActions.ts` (guard statements only, excluding `pull`).
- Create/Test: `src/lib/gitActions.notices.test.ts`.

**Interfaces:**
- Consumes: `reportNotice(title: string, message: string): void`; PR 1's existing `reportError` import and wrapper failure behavior.
- Produces: module-private `cantTitle(label: string): string` — "Can't " + the label with only its first character lowercased, so ref and remote names inside a label keep their case.
- Produces: unchanged `run(label: string, fn: () => Promise<unknown>): Promise<boolean>`, `runOp(label: string, fn: () => Promise<OpOutcome>): Promise<OpOutcome | null>`, `runResolve(label: string, fn: () => Promise<unknown>): Promise<boolean>`, `runWorktree(label: string, fn: () => Promise<unknown>): Promise<boolean>`.
- Produces: unchanged `runDestructive(label: string, consequence: string, fn: (backup: boolean) => Promise<RewriteResult>): Promise<boolean>`, `runDestructiveRebase(label: string, consequence: string, fn: (backup: boolean) => Promise<RebaseOutcome>): Promise<boolean>`, `runRemote(label: string, fn: (onLine: (l: string) => void, creds?: { username: string; password: string }) => Promise<RemoteOutcome>): Promise<boolean>`, `sameRepoAfterPrompt(): () => boolean`.
- Direct action returns remain unchanged: `fetch(): Promise<boolean>`, `push(forceWithLease?: boolean): Promise<boolean>`, `undo(): Promise<boolean>`, `squashMerge(reference: string): Promise<void>`, `createPullRequest(): Promise<void>`.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/gitActions.notices.test.ts`:

```ts
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { appState } from "./store.svelte";
import { errors } from "./errors.svelte";
import { dialogs } from "./dialogs.svelte";
import { gitActions } from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";

vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = "";
  appState.setRepoLoading(false);
  appState.status = "Ready";
});
afterEach(() => {
  dialogs.resolveConfirm(false);
  dialogs.resolveDestructive(false);
  errors.reset();
  appState.repo = "";
  appState.isRewriting = false;
  appState.endRemoteProgress();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const noRepo = [
  { title: "Can't checkout main", call: () => gitActions.checkout("main") },
  // Only the label's first character is lowercased — the ref name keeps its case.
  { title: "Can't checkout Feature/ABC", call: () => gitActions.checkout("Feature/ABC") },
  { title: "Can't merge feature", call: () => gitActions.merge("feature") },
  { title: "Can't keep f", call: () => gitActions.resolveKeep("f") },
  { title: "Can't stage 1 file(s)", call: () => gitActions.stage(["f"]) },
  { title: "Can't reset (mixed)", call: () => gitActions.reset("a", "mixed", "Reset") },
  { title: "Can't rebase onto a", call: () => gitActions.rebaseOnto("a", "Rebase") },
  { title: "Can't push", call: () => gitActions.push() },
  { title: "Can't discard the hunk", call: () => gitActions.discardHunk("f", 0, 1, "patch") },
  { title: "Can't discard the lines", call: () => gitActions.discardLines("f", 0, [0], "patch") },
  { title: "Can't amend the commit", call: () => gitActions.amendCommit("message") },
  { title: "Can't discard changes", call: () => gitActions.discard(["f"]) },
  { title: "Can't remove untracked files", call: () => gitActions.clean(["f"]) },
];
it.each(noRepo)("notices missing repository: $title", async ({ title, call }) => {
  await call();
  expect(errors.current).toMatchObject({ kind: "notice", title, headline: "Open a repository first." });
  expect(errors.pending).toBe(1);
  expect(appState.status).toBe("Open a repository first.");
  expect(appState.statusIsError).toBe(false);
});
it.each(noRepo)("keeps preview guard status-only: $title", async ({ call }) => {
  vi.stubGlobal("window", {});
  await call();
  expect(errors.pending).toBe(0);
  expect(appState.status).toBe("That action needs the desktop app (not the browser preview).");
});

it.each(["fetch", "push"] as const)("notices a loading repository for %s", async (action) => {
  appState.repo = "/tmp/loading";
  appState.setRepoLoading(true);
  const operation = vi.spyOn(api, action);
  await gitActions[action]();
  expect(operation).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: `Can't ${action} yet`, headline: "Repository is still loading — try again in a moment." });
});
it("notices a concurrent remote operation", async () => {
  appState.repo = "/tmp/repo";
  appState.setRepoLoading(false);
  appState.startRemoteProgress("Fetch");
  await gitActions.push();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't push yet", headline: "A remote operation is already in progress." });
});
it.each(["remote", "rewrite"])("notices squash merge blocked by %s", async (busy) => {
  appState.repo = "/tmp/repo";
  if (busy === "remote") appState.startRemoteProgress("Push");
  else appState.isRewriting = true;
  const merge = vi.spyOn(api, "merge");
  await gitActions.squashMerge("feature");
  expect(merge).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't squash merge yet", headline: "Wait for the current remote operation or history rewrite to finish." });
});
it("keeps squash merge's combined preview guard status-only", async () => {
  await gitActions.squashMerge("feature");
  expect(appState.status).toBe("Squash merge needs the desktop app and an open repository.");
  expect(errors.pending).toBe(0);
});
it("notices a repository switch during confirmation", async () => {
  appState.repo = "/tmp/first";
  const discard = vi.spyOn(api, "discard");
  const pending = gitActions.discard(["f"]);
  expect(dialogs.state.kind).toBe("confirm");
  appState.repo = "/tmp/second";
  dialogs.resolveConfirm(true);
  expect(await pending).toBe(false);
  expect(discard).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Repository changed", headline: "Repository changed while that dialog was open — nothing was done." });
});
it("notices invalid undo and clears the snapshot", async () => {
  appState.repo = "/tmp/repo";
  appState.setGraphCommits([]);
  appState.setRefsDetailed([]);
  appState.setLastUndo({ branch: "main", sha: "a", label: "Reset" });
  expect(await gitActions.undo()).toBe(false);
  expect(appState.lastUndo).toBeNull();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't undo", headline: "Undo unavailable — the checked-out branch changed." });
});
it.each([false, true])("notices create-PR refusal with checked-out branch=%s", async (hasBranch) => {
  appState.repo = "/tmp/repo";
  appState.setGraphCommits(hasBranch ? [{ ...SAMPLE_GRAPH[0], refs: [{ name: "main", kind: "local", is_head: true }] }] : []);
  appState.setRefsDetailed(hasBranch ? [{ name: "main", kind: "local", target_sha: SAMPLE_GRAPH[0].sha, upstream: null, ahead: 0, behind: 0 }] : []);
  await gitActions.createPullRequest();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't create a pull request", headline: hasBranch ? "No other local branch to use as the PR base." : "No branch checked out — check out a branch to create a pull request." });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/gitActions.notices.test.ts`

Expected: FAIL on notice assertions (`errors.current` is null); the currently silent resolve guard also lacks the browser-preview status. No backend mutation should be invoked by these tests.

- [ ] **Step 3: Implement only the guards**

Extend the PR 1 import in `src/lib/gitActions.ts` to:

```ts
import { reportError, reportNotice } from "./errors.svelte";
```

PR 1 changes the wrappers' failure catches. Preserve those catches and their refresh behavior. The following are exact guard replacements, not whole-wrapper replacements.

Labels embed names the user typed (`Checkout ${target}`, `Rename ${oldName} → ${newName}`, `Delete remote branch ${remote}/${branch}`), so a title must not lowercase the whole label. Add the helper directly above `run`. PR 1 leaves this comment line untouched:

**Before**
```ts
// Run an op with uniform guard / status / refresh / error handling.
```

**After**
```ts
// Title for a refusal notice ("Can't checkout Feature/ABC"). Only the label's first character
// is lowercased: the rest carries ref and remote names exactly as the user typed them.
function cantTitle(label: string): string {
  return `Can't ${label.charAt(0).toLowerCase()}${label.slice(1)}`;
}

// Run an op with uniform guard / status / refresh / error handling.
```

In `src/lib/gitActions.ts`, inside **each** of `run`, `runOp`, `runWorktree`, `runDestructive`, `runDestructiveRebase`, and `runRemote` (six occurrences in these named functions only):

**Before**
```ts
    appState.status = "Open a repository first.";
```

**After**
```ts
    reportNotice(cantTitle(label), "Open a repository first.");
```

In `src/lib/gitActions.ts`, `runResolve`:

**Before**
```ts
  if (!isTauri() || !appState.repo) return false;
```

**After**
```ts
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return false;
  }
  if (!appState.repo) {
    reportNotice(cantTitle(label), "Open a repository first.");
    return false;
  }
```

In `src/lib/gitActions.ts`, `discardHunk` only:

**Before**
```ts
      appState.status = "Open a repository first.";
```

**After**
```ts
      reportNotice("Can't discard the hunk", "Open a repository first.");
```

In `src/lib/gitActions.ts`, `discardLines` only:

**Before**
```ts
      appState.status = "Open a repository first.";
```

**After**
```ts
      reportNotice("Can't discard the lines", "Open a repository first.");
```

In `src/lib/gitActions.ts`, `amendCommit` only:

**Before**
```ts
      appState.status = "Open a repository first.";
```

**After**
```ts
      reportNotice("Can't amend the commit", "Open a repository first.");
```

In `src/lib/gitActions.ts`, `discard` only:

**Before**
```ts
      appState.status = "Open a repository first.";
```

**After**
```ts
      reportNotice("Can't discard changes", "Open a repository first.");
```

In `src/lib/gitActions.ts`, `clean` only:

**Before**
```ts
      appState.status = "Open a repository first.";
```

**After**
```ts
      reportNotice("Can't remove untracked files", "Open a repository first.");
```

In `src/lib/gitActions.ts`, `fetch` only; do not replace the matching text in `pull`:

**Before**
```ts
      appState.status = "Repository is still loading — try again in a moment.";
```

**After**
```ts
      reportNotice("Can't fetch yet", "Repository is still loading — try again in a moment.");
```

In `src/lib/gitActions.ts`, `push` only; do not replace the matching text in `pull`:

**Before**
```ts
      appState.status = "Repository is still loading — try again in a moment.";
```

**After**
```ts
      reportNotice("Can't push yet", "Repository is still loading — try again in a moment.");
```

In `src/lib/gitActions.ts`, `runRemote`:

**Before**
```ts
  if (appState.remoteOpActive) { appState.status = "A remote operation is already in progress."; return false; }
```

**After**
```ts
  if (appState.remoteOpActive) {
    reportNotice(`${cantTitle(label)} yet`, "A remote operation is already in progress.");
    return false;
  }
```

In `src/lib/gitActions.ts`, `sameRepoAfterPrompt`:

**Before**
```ts
    appState.status = "Repository changed while that dialog was open — nothing was done.";
```

**After**
```ts
    reportNotice("Repository changed", "Repository changed while that dialog was open — nothing was done.");
```

In `src/lib/gitActions.ts`, `undo`:

**Before**
```ts
      appState.status = "Undo unavailable — the checked-out branch changed.";
```

**After**
```ts
      reportNotice("Can't undo", "Undo unavailable — the checked-out branch changed.");
```

In `src/lib/gitActions.ts`, `createPullRequest`:

**Before**
```ts
      appState.status = "No branch checked out — check out a branch to create a pull request.";
```

**After**
```ts
      reportNotice("Can't create a pull request", "No branch checked out — check out a branch to create a pull request.");
```

In `src/lib/gitActions.ts`, `createPullRequest`:

**Before**
```ts
      appState.status = "No other local branch to use as the PR base.";
```

**After**
```ts
      reportNotice("Can't create a pull request", "No other local branch to use as the PR base.");
```

In `src/lib/gitActions.ts`, `squashMerge` (leave its preceding combined desktop/repo status-only guard intact):

**Before**
```ts
    if (appState.remoteOpActive || appState.isRewriting) return;
```

**After**
```ts
    if (appState.remoteOpActive || appState.isRewriting) {
      reportNotice("Can't squash merge yet", "Wait for the current remote operation or history rewrite to finish.");
      return;
    }
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/gitActions.notices.test.ts src/lib/gitActions.partialFailure.test.ts`

Expected: PASS; notices retain their body text and normal status color, a mixed-case ref name survives in its title, and existing partial-failure refresh tests remain green.

- [ ] **Step 5: Commit**

```bash
git add src/lib/gitActions.ts src/lib/gitActions.notices.test.ts
git commit -F - <<'COMMIT'
fix: surface operation refusals as notices

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 3: Handle reword precheck errors and refusals

**Files:**
- Modify: `src/lib/gitActions.ts` (`setCommitMessage`).
- Create/Test: `src/lib/gitActions.reword.test.ts`.

**Interfaces:**
- Consumes: `api.countMergesInRange(repo: string, base: string): Promise<number>`; `api.rebaseTodoPreview(repo: string, base: string): Promise<ReflogEntry[]>`; `gitActions.amend(message: string | null, resetAuthorDate: boolean, resetCommitterDate: boolean): Promise<boolean>`; `gitActions.rebaseInteractive(base: string, steps: RebaseStep[], consequence: string): Promise<boolean>`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`; `reportNotice(title: string, message: string): void`.
- Produces: unchanged `gitActions.setCommitMessage(commit: GraphCommit, message: string): Promise<void>`. It consumes rejected prechecks and leaves wrapper-owned failure reporting alone.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/gitActions.reword.test.ts`:

```ts
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { appState } from "./store.svelte";
import { errors, reportError } from "./errors.svelte";
import { gitActions } from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";
import type { GraphCommit } from "./types";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
const commit: GraphCommit = { ...SAMPLE_GRAPH[0], sha: "target", parents: ["base"], refs: [] };
beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = "/tmp/reword";
  appState.setRepoLoading(false);
  vi.spyOn(api, "countMergesInRange").mockResolvedValue(0);
  vi.spyOn(api, "rebaseTodoPreview").mockResolvedValue([{ sha: "target", short: "target", selector: "", subject: "Old" }]);
});
afterEach(() => {
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
it.each(["countMergesInRange", "rebaseTodoPreview"] as const)("reports %s rejection once", async (method) => {
  vi.mocked(api[method]).mockRejectedValue(new Error("fatal: object missing"));
  const rebase = vi.spyOn(gitActions, "rebaseInteractive");
  await expect(gitActions.setCommitMessage(commit, "New")).resolves.toBeUndefined();
  expect(rebase).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't reword the commit", headline: "object missing" });
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
});
it.each([
  ["root", "Can't reword the root commit here."],
  ["merge", "Can't reword a merge commit here."],
  ["range", "Can't reword across a merge commit yet."],
  ["missing", "Can't reword that commit — it isn't in the current branch's history."],
])("notices %s refusal", async (kind, headline) => {
  const target = { ...commit, parents: kind === "root" ? [] : kind === "merge" ? ["a", "b"] : ["base"] };
  if (kind === "range") vi.mocked(api.countMergesInRange).mockResolvedValue(1);
  if (kind === "missing") vi.mocked(api.rebaseTodoPreview).mockResolvedValue([]);
  await gitActions.setCommitMessage(target, "New");
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't reword this commit", headline });
  expect(appState.status).toBe(headline);
  expect(appState.statusIsError).toBe(false);
});
it.each([false, true])("does not re-report a wrapper-owned failure, HEAD=%s", async (head) => {
  const owned = async () => { reportError("Owned failure", "fatal: rejected"); return false; };
  vi.spyOn(gitActions, "amend").mockImplementation(owned);
  vi.spyOn(gitActions, "rebaseInteractive").mockImplementation(owned);
  await gitActions.setCommitMessage({ ...commit, refs: head ? [{ name: "main", kind: "local", is_head: true }] : [] }, "New");
  expect(errors.pending).toBe(1);
  expect(errors.current?.title).toBe("Owned failure");
});
it("notices missing repository without invoking either precheck", async () => {
  appState.repo = "";
  await gitActions.setCommitMessage(commit, "New");
  expect(api.countMergesInRange).not.toHaveBeenCalled();
  expect(api.rebaseTodoPreview).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", headline: "Open a repository first." });
});
it("keeps browser preview status-only", async () => {
  vi.stubGlobal("window", {});
  await gitActions.setCommitMessage(commit, "New");
  expect(api.countMergesInRange).not.toHaveBeenCalled();
  expect(errors.pending).toBe(0);
  expect(appState.status).toBe("That action needs the desktop app (not the browser preview).");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/gitActions.reword.test.ts`

Expected: FAIL because the precheck promise rejects and the four refusals do not queue notices.

- [ ] **Step 3: Implement the reword boundary**

Use Task 2's existing `reportError, reportNotice` import. Replace the whole `setCommitMessage` property below. The catches inside `amend` and `rebaseInteractive` belong to PR 1 and are not changed.

**Before** (`src/lib/gitActions.ts`)
```ts
  setCommitMessage: async (commit: GraphCommit, message: string): Promise<void> => {
    if (commit.refs.some((r) => r.is_head)) {
      // HEAD: amend in place. Do NOT reset author/committer dates.
      await gitActions.amend(message, false, false);
      return;
    }
    // Non-HEAD reword: needs a linear base..HEAD replay, so reject merges/root.
    if (commit.parents.length === 0) {
      appState.status = "Can't reword the root commit here.";
      return;
    }
    if (commit.parents.length > 1) {
      appState.status = "Can't reword a merge commit here.";
      return;
    }
    const base = commit.parents[0];
    // A reword replays base..HEAD with `pick`, and git refuses `pick <merge>`,
    // so refuse up-front if a merge sits anywhere in that range (otherwise the
    // rebase fails and strands the repo mid-operation).
    if ((await api.countMergesInRange(appState.repo, base)) > 0) {
      appState.status = "Can't reword across a merge commit yet.";
      return;
    }
    const preview = await api.rebaseTodoPreview(appState.repo, base);
    if (!preview.some((e) => e.sha === commit.sha)) {
      appState.status = "Can't reword that commit — it isn't in the current branch's history.";
      return;
    }
    const steps: RebaseStep[] = preview.map((e) => ({
      action: e.sha === commit.sha ? "reword" : "pick",
      sha: e.sha,
      message: e.sha === commit.sha ? message : null,
    }));
    await gitActions.rebaseInteractive(
      base,
      steps,
      `Reword ${commit.sha.slice(0, 9)} — rewrites it and the ${preview.length - 1} commit(s) after it; hashes change.`,
    );
  },
```

**After**
```ts
  setCommitMessage: async (commit: GraphCommit, message: string): Promise<void> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return;
    }
    if (!appState.repo) {
      reportNotice("Can't reword this commit", "Open a repository first.");
      return;
    }
    if (commit.refs.some((r) => r.is_head)) {
      // HEAD: amend in place. Do NOT reset author/committer dates.
      await gitActions.amend(message, false, false);
      return;
    }
    // Non-HEAD reword: needs a linear base..HEAD replay, so reject merges/root.
    if (commit.parents.length === 0) {
      reportNotice("Can't reword this commit", "Can't reword the root commit here.");
      return;
    }
    if (commit.parents.length > 1) {
      reportNotice("Can't reword this commit", "Can't reword a merge commit here.");
      return;
    }
    const base = commit.parents[0];
    let preview: Awaited<ReturnType<typeof api.rebaseTodoPreview>>;
    try {
      // A reword replays base..HEAD with `pick`, and git refuses `pick <merge>`,
      // so refuse up-front if a merge sits anywhere in that range (otherwise the
      // rebase fails and strands the repo mid-operation).
      if ((await api.countMergesInRange(appState.repo, base)) > 0) {
        reportNotice("Can't reword this commit", "Can't reword across a merge commit yet.");
        return;
      }
      preview = await api.rebaseTodoPreview(appState.repo, base);
    } catch (e) {
      reportError("Couldn't reword the commit", e);
      return;
    }
    if (!preview.some((e) => e.sha === commit.sha)) {
      reportNotice("Can't reword this commit", "Can't reword that commit — it isn't in the current branch's history.");
      return;
    }
    const steps: RebaseStep[] = preview.map((e) => ({
      action: e.sha === commit.sha ? "reword" : "pick",
      sha: e.sha,
      message: e.sha === commit.sha ? message : null,
    }));
    await gitActions.rebaseInteractive(
      base,
      steps,
      `Reword ${commit.sha.slice(0, 9)} — rewrites it and the ${preview.length - 1} commit(s) after it; hashes change.`,
    );
  },
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/gitActions.reword.test.ts src/lib/gitActions.notices.test.ts`

Expected: PASS. No catch wraps a wrapper's already-reported `false` result, and the original comment above the merge-range check is kept word for word. Keep `CommitMessageEdit.updateMessage`'s existing `onDone` behavior; preserving typed input is a deferred spec item.

- [ ] **Step 5: Commit**

```bash
git add src/lib/gitActions.ts src/lib/gitActions.reword.test.ts
git commit -F - <<'COMMIT'
fix: report reword precheck failures

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 4: Report commit-message loading failures

**Files:**
- Modify: `src/lib/components/CommitComposer.svelte` (`onAmendToggle`).
- Modify: `src/lib/components/CommitMessageEdit.svelte` (message-load catch).
- Create/Test: `src/lib/components/CommitMessage.coverage.test.ts`.

**Interfaces:**
- Consumes: `gitActions.getCommitMessage(sha: string): Promise<string>`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`.
- Produces: unchanged component props (`CommitComposer` has none; `CommitMessageEdit` has `onDone?: () => void`), `onAmendToggle(e: Event): Promise<void>` with preserved draft restoration and stale-token guards.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/CommitMessage.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { gitActions } from "../gitActions";
import { SAMPLE_GRAPH } from "../graph/sample";
import CommitComposer from "./CommitComposer.svelte";
import CommitMessageEdit from "./CommitMessageEdit.svelte";

let dispose: (() => Promise<void>) | undefined;
beforeEach(() => {
  appState.setGraphCommits([{ ...SAMPLE_GRAPH[0], refs: [{ name: "main", kind: "local", is_head: true }] }]);
  appState.selected = new Set([SAMPLE_GRAPH[0].sha]);
  vi.spyOn(gitActions, "getCommitMessage").mockRejectedValue(new Error("fatal: missing object"));
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.setGraphCommits([]);
  document.body.replaceChildren();
  vi.restoreAllMocks();
});
it("reports amend loading failure, unticks, and keeps the user's draft", async () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(CommitComposer, { target });
  dispose = () => unmount(panel);
  const title = target.querySelector<HTMLInputElement>(".title-box")!;
  title.value = "My draft";
  title.dispatchEvent(new Event("input", { bubbles: true }));
  await tick();
  const checkbox = target.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
  checkbox.checked = true;
  checkbox.dispatchEvent(new Event("change", { bubbles: true }));
  await vi.waitFor(() => expect(errors.current?.headline).toBe("missing object"));
  await tick();
  expect(errors.current?.title).toBe("Couldn't load the commit message");
  expect(errors.pending).toBe(1);
  expect(checkbox.checked).toBe(false);
  expect(title.value).toBe("My draft");
});
it("reports selected commit-message loading failure", async () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(CommitMessageEdit, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't load the commit message", headline: "missing object" }));
  expect(appState.statusIsError).toBe(true);
});
it("ignores an obsolete amend load failure after the user unticks", async () => {
  let reject!: (error: unknown) => void;
  vi.mocked(gitActions.getCommitMessage).mockImplementation(() => new Promise<string>((_, fail) => { reject = fail; }));
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(CommitComposer, { target });
  dispose = () => unmount(panel);
  const checkbox = target.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
  checkbox.checked = true;
  checkbox.dispatchEvent(new Event("change", { bubbles: true }));
  await tick();
  checkbox.checked = false;
  checkbox.dispatchEvent(new Event("change", { bubbles: true }));
  reject(new Error("old failure"));
  await tick();
  await Promise.resolve();
  expect(errors.pending).toBe(0);
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/CommitMessage.coverage.test.ts`

Expected: FAIL because both load failures leave `errors.current` null.

- [ ] **Step 3: Implement the two reporting catches**

`src/lib/components/CommitComposer.svelte` — **Before**
```ts
  import { gitActions } from "../gitActions";
```

**After**
```ts
  import { gitActions } from "../gitActions";
  import { reportError } from "../errors.svelte";
```

`src/lib/components/CommitComposer.svelte` — **Before**
```ts
      } catch {
        // Couldn't read HEAD's message — back out of amend cleanly.
        if (token === amendToken) {
          amend = false;
          title = draftTitle;
          body = draftBody;
          draftTitle = "";
          draftBody = "";
        }
      }
```

**After**
```ts
      } catch (e) {
        // Couldn't read HEAD's message — back out of amend cleanly.
        if (token === amendToken) {
          reportError("Couldn't load the commit message", e);
          amend = false;
          title = draftTitle;
          body = draftBody;
          draftTitle = "";
          draftBody = "";
        }
      }
```

`src/lib/components/CommitMessageEdit.svelte` — **Before**
```ts
  import { gitActions } from "../gitActions";
```

**After**
```ts
  import { gitActions } from "../gitActions";
  import { reportError } from "../errors.svelte";
```

`src/lib/components/CommitMessageEdit.svelte` — **Before**
```ts
      .catch(() => {
        if (seededSha === sha) appState.status = "Couldn't load the commit message.";
      })
```

**After**
```ts
      .catch((e) => {
        if (seededSha === sha) reportError("Couldn't load the commit message", e);
      })
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/CommitMessage.coverage.test.ts src/lib/gitActions.reword.test.ts`

Expected: PASS; current loads report once and obsolete amend failures remain ignored.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/CommitComposer.svelte src/lib/components/CommitMessageEdit.svelte src/lib/components/CommitMessage.coverage.test.ts
git commit -F - <<'COMMIT'
fix: surface commit message loading failures

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 5: Report native file-opening and SHA-copy failures

**Files:**
- Modify: `src/lib/components/WorkingCopyView.svelte` (file-action callbacks only).
- Modify: `src/lib/components/WorktreePanel.svelte` (folder callback).
- Modify: `src/lib/components/GraphHistory.svelte` (Copy SHA callback).
- Modify: `src/lib/components/github/PrCommitsTab.svelte` (`copySha`).
- Create/Test: `src/lib/components/NativeActions.coverage.test.ts`.

**Interfaces:**
- Consumes: `openPath(path: string, openWith?: string): Promise<void>`; `revealItemInDir(path: string | string[]): Promise<void>`; `navigator.clipboard.writeText(data: string): Promise<void>`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`.
- Produces: unchanged context-menu `MenuItem.action?: () => void`; `PrCommitsTab` props `{ commits: GhCommit[] }` and local `copySha(oid: string): Promise<void>` preserve success feedback. No general-purpose native-action abstraction is added.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/NativeActions.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { contextMenu } from "../contextMenu.svelte";
import { SAMPLE_GRAPH } from "../graph/sample";
import WorkingCopyView from "./WorkingCopyView.svelte";
import WorktreePanel from "./WorktreePanel.svelte";
import GraphHistory from "./GraphHistory.svelte";
import PrCommitsTab from "./github/PrCommitsTab.svelte";

vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn(), revealItemInDir: vi.fn() }));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  // DiffView reads window.matchMedia at init and jsdom has none; a right-click selects the
  // file, its (empty) diff loads, and DiffView mounts.
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  appState.repo = "/tmp/repo";
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH.slice(0, 1));
  appState.setFileTreeView(false);
  vi.spyOn(api, "diff").mockResolvedValue("");
  vi.spyOn(api, "appsForFile").mockResolvedValue([{ name: "TextEdit", path: "/Applications/TextEdit.app" }]);
  vi.mocked(openPath).mockReset().mockRejectedValue(new Error("fatal: launch denied"));
  vi.mocked(revealItemInDir).mockReset().mockRejectedValue(new Error("fatal: launch denied"));
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockRejectedValue(new Error("fatal: copy denied")) } });
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  contextMenu.close();
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  if (clipboard) Object.defineProperty(navigator, "clipboard", clipboard);
  else Reflect.deleteProperty(navigator, "clipboard");
});

it.each([
  ["Open", false, "Couldn't open the file"],
  ["Open With", false, "Couldn't open the file"],
  ["Show in Finder", false, "Couldn't show in Finder"],
  ["Show in Finder", true, "Couldn't show in Finder"],
] as const)("reports %s failure (deleted=%s)", async (label, deleted, title) => {
  appState.setWorkingChanges([{ path: "f.txt", status: deleted ? "deleted" : "modified", staged: false, unstaged: true, untracked: false, conflicted: false }]);
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector(".file-row")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(contextMenu.open).toBe(true));
  const item = contextMenu.items.find((item) => item.label === label)!;
  (label === "Open With" ? item.submenu![0] : item).action!();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title, headline: "launch denied" }));
  expect(errors.pending).toBe(1);
  if (label === "Open With") expect(openPath).toHaveBeenCalledWith("/tmp/repo/f.txt", "/Applications/TextEdit.app");
  if (label === "Show in Finder" && deleted) expect(openPath).toHaveBeenCalledWith("/tmp/repo");
  if (label === "Show in Finder" && !deleted) expect(revealItemInDir).toHaveBeenCalledWith("/tmp/repo/f.txt");
});
it("keeps failed app discovery as silent submenu omission", async () => {
  vi.mocked(api.appsForFile).mockRejectedValue(new Error("discovery failed"));
  appState.setWorkingChanges([{ path: "f.txt", status: "modified", staged: false, unstaged: true, untracked: false, conflicted: false }]);
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector(".file-row")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(contextMenu.open).toBe(true));
  expect(contextMenu.items.some((item) => item.label === "Open With")).toBe(false);
  expect(errors.pending).toBe(0);
});
it("reports Open Worktree Folder failure", async () => {
  const panel = mount(WorktreePanel, { target, props: { worktrees: [{ path: "/tmp/linked", head: "a", branch: "feature", isMain: false, isCurrent: false, detached: false, bare: false, locked: false, lockedReason: null, prunable: false, prunableReason: null, status: null }] } });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector(".worktree")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  contextMenu.items.find((item) => item.label === "Open Worktree Folder")!.action!();
  await vi.waitFor(() => expect(errors.current?.title).toBe("Couldn't open the worktree folder"));
});
it.each([false, true])("reports graph copy failure, missing clipboard=%s", async (missing) => {
  if (missing) Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
  const panel = mount(GraphHistory, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector('[id^="gc-row-"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  contextMenu.items.find((item) => item.label === "Copy SHA")!.action!();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't copy the SHA", headline: missing ? "Clipboard is unavailable." : "copy denied" }));
});
it.each([false, true])("reports PR copy failure without success feedback, missing clipboard=%s", async (missing) => {
  if (missing) Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
  const panel = mount(PrCommitsTab, { target, props: { commits: [{ oid: "a".repeat(40), message: "Subject", author: "A", committedDate: "2026-09-26T00:00:00Z" }] } });
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>('[title="Copy full SHA"]')!.click();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't copy the SHA", headline: missing ? "Clipboard is unavailable." : "copy denied" }));
  expect(target.textContent).not.toContain("✓ copied");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/NativeActions.coverage.test.ts`

Expected: FAIL because native actions reject without queueing reports and clipboard failures are swallowed. Vitest may also flag the uncaught native promises.

- [ ] **Step 3: Implement each action catch**

`src/lib/components/WorkingCopyView.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportError } from "../errors.svelte";
```

`src/lib/components/WorktreePanel.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportError } from "../errors.svelte";
```

`src/lib/components/GraphHistory.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportError } from "../errors.svelte";
```

`src/lib/components/WorkingCopyView.svelte` — **Before**
```ts
      fileItems.push({ label: "Open", disabled: gone, action: () => void openPath(abs) });
```

**After**
```ts
      fileItems.push({
        label: "Open",
        disabled: gone,
        action: () => void openPath(abs).catch((e) => reportError("Couldn't open the file", e)),
      });
```

`src/lib/components/WorkingCopyView.svelte` — **Before**
```ts
            submenu: apps.map((a) => ({ label: a.name, action: () => void openPath(abs, a.path) })),
```

**After**
```ts
            submenu: apps.map((a) => ({
              label: a.name,
              action: () => void openPath(abs, a.path).catch((e) => reportError("Couldn't open the file", e)),
            })),
```

`src/lib/components/WorkingCopyView.svelte` — **Before**
```ts
        action: () => void (gone ? openPath(parent) : revealItemInDir(abs)),
```

**After**
```ts
        action: () => void (gone ? openPath(parent) : revealItemInDir(abs))
          .catch((e) => reportError("Couldn't show in Finder", e)),
```

`src/lib/components/WorktreePanel.svelte` — **Before**
```ts
      { label: "Open Worktree Folder", action: () => void openPath(worktree.path) },
```

**After**
```ts
      {
        label: "Open Worktree Folder",
        action: () => void openPath(worktree.path).catch((e) => reportError("Couldn't open the worktree folder", e)),
      },
```

`src/lib/components/GraphHistory.svelte` — **Before**
```ts
      { label: "Copy SHA", action: () => navigator.clipboard?.writeText(sha) },
```

**After**
```ts
      {
        label: "Copy SHA",
        action: async () => {
          if (!navigator.clipboard) {
            reportError("Couldn't copy the SHA", "Clipboard is unavailable.");
            return;
          }
          try {
            await navigator.clipboard.writeText(sha);
          } catch (e) {
            reportError("Couldn't copy the SHA", e);
          }
        },
      },
```

`src/lib/components/github/PrCommitsTab.svelte` — **Before**
```ts
  import { appState } from "../../store.svelte";
```

**After**
```ts
  import { appState } from "../../store.svelte";
  import { reportError } from "../../errors.svelte";
```

`src/lib/components/github/PrCommitsTab.svelte` — **Before**
```ts
  async function copySha(oid: string) {
    try {
      await navigator.clipboard.writeText(oid);
      copiedOid = oid;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedOid = null), 1200);
    } catch {
      /* clipboard unavailable — leave the chip as-is */
    }
  }
```

**After**
```ts
  async function copySha(oid: string) {
    if (!navigator.clipboard) {
      reportError("Couldn't copy the SHA", "Clipboard is unavailable.");
      return;
    }
    try {
      await navigator.clipboard.writeText(oid);
      copiedOid = oid;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedOid = null), 1200);
    } catch (e) {
      reportError("Couldn't copy the SHA", e);
    }
  }
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/NativeActions.coverage.test.ts src/lib/components/WorktreePanel.test.ts`

Expected: PASS, with one report for each chosen action failure and none for app discovery. No real opener or clipboard is called.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/WorkingCopyView.svelte src/lib/components/WorktreePanel.svelte src/lib/components/GraphHistory.svelte src/lib/components/github/PrCommitsTab.svelte src/lib/components/NativeActions.coverage.test.ts
git commit -F - <<'COMMIT'
fix: report file opening and sha copy failures

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 6: Surface history-edit failures and validation notices

**Files:**
- Modify: `src/lib/components/ApplyPanel.svelte` (`rewrite`).
- Modify: `src/lib/components/EditTabs.svelte` (`needsSelection`, `previewExact`, `previewCompress`).
- Modify: `src/lib/components/RebaseTodo.svelte` (`startRebase`).
- Create/Test: `src/lib/components/EditFailures.coverage.test.ts`.

**Interfaces:**
- Consumes: `api.rewriteHistory(repo: string, mappings: DateMapping[], options: RewriteOptions, onEvent: (line: string) => void): Promise<void>`; `prerequisites.canEditHistory: boolean`; `api.rebaseTodoPreview(repo: string, base: string): Promise<ReflogEntry[]>`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`; `reportNotice(title: string, message: string): void`.
- Produces: unchanged local `rewrite(): Promise<void>`, `needsSelection(): string[] | null`, `previewExact(): void`, `previewCompress(): void`, `startRebase(): Promise<void>`; no new public interface.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/EditFailures.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { prerequisites } from "../prerequisites.svelte";
import { rebaseEditor } from "../rebaseEditor.svelte";
import { SAMPLE_GRAPH } from "../graph/sample";
import ApplyPanel from "./ApplyPanel.svelte";
import EditTabs from "./EditTabs.svelte";
import RebaseTodo from "./RebaseTodo.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let target: HTMLDivElement;
let dispose: (() => Promise<void>) | undefined;
function button(text: string): HTMLButtonElement {
  const found = Array.from(target.querySelectorAll("button")).find((b) => b.textContent?.trim() === text);
  if (!found) throw new Error(`Missing button: ${text}`);
  return found;
}
function numberValues(values: Array<number | string>) {
  const inputs = target.querySelectorAll<HTMLInputElement>('input[type="number"]');
  values.forEach((value, i) => {
    inputs[i].value = String(value);
    inputs[i].dispatchEvent(new Event("input", { bubbles: true }));
  });
}
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/edits";
  appState.setGraphCommits(SAMPLE_GRAPH);
  appState.clearNewDates();
  appState.clearLog();
  vi.spyOn(prerequisites, "load").mockResolvedValue(undefined);
  vi.spyOn(prerequisites, "canEditHistory", "get").mockReturnValue(true);
  vi.stubGlobal("confirm", vi.fn(() => true));
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  rebaseEditor.close();
  errors.reset();
  appState.repo = "";
  appState.isRewriting = false;
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("reports rewrite failure once, preserves progress logging and queued dates", async () => {
  vi.spyOn(api, "rewriteHistory").mockImplementation(async (_repo, _mappings, _options, onLine) => {
    onLine("Scanning commits");
    throw new Error("fatal: rewrite denied");
  });
  appState.setNewDate(SAMPLE_GRAPH[0].sha, new Date("2026-01-01T00:00:00Z"));
  const panel = mount(ApplyPanel, { target });
  dispose = () => unmount(panel);
  await tick();
  button("Rewrite history").click();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Rewrite failed", headline: "rewrite denied" }));
  expect(errors.pending).toBe(1);
  expect(appState.logLines).toContain("Scanning commits");
  expect(appState.logLines).toContain("[rewrite] stopped.");
  expect(appState.logLines.join("\n").match(/rewrite denied/g)).toHaveLength(1);
  expect(appState.newDates.size).toBe(1);
  expect(appState.isRewriting).toBe(false);
});
it.each([false, true])("notices ApplyPanel guard, prerequisites=%s", async (ready) => {
  vi.spyOn(prerequisites, "canEditHistory", "get").mockReturnValue(ready);
  const rewrite = vi.spyOn(api, "rewriteHistory");
  const panel = mount(ApplyPanel, { target });
  dispose = () => unmount(panel);
  await tick();
  // Exercise the defensive handler as if a stale enabled control dispatched it.
  button("Rewrite history").disabled = false;
  button("Rewrite history").click();
  expect(rewrite).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't rewrite history", headline: ready ? "No new dates queued. Use an edit mode to preview changes first." : "Commit-time editing is unavailable — the bundled git-filter-repo couldn't run." });
});
it.each([
  ["selection", "Select one or more commits first."],
  ["exact", "Enter a complete, valid date."],
  ["count", "Compress needs at least 2 selected commits."],
  ["incomplete", "Enter a complete, valid date."],
  ["order", "Compress: end must be after start."],
  ["unreadable", "Compress: couldn't read dates for the selected commits."],
])("notices date validation: %s", async (kind, headline) => {
  const panel = mount(EditTabs, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  appState.selected = new Set(kind === "selection" ? [] : kind === "count" ? ["a"] : ["a", "b"]);
  if (kind === "exact") {
    button("Exact").click();
    await tick();
    numberValues([""]);
  } else if (kind !== "selection") {
    button("Compress").click();
    await tick();
    numberValues([2026, 9, 26, 12, 0, 0, 2026, 9, 25, 12, 0, 0]);
    if (kind === "incomplete") numberValues([""]);
    if (kind === "order") numberValues([2026, 9, 24]);
    if (kind === "unreadable") appState.commits = [];
  }
  await tick();
  button("Preview").click();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't preview date changes", headline });
  expect(appState.status).toBe(headline);
  expect(appState.statusIsError).toBe(false);
});
it("notices all-drop interactive rebase without closing its editor", async () => {
  vi.spyOn(api, "rebaseTodoPreview").mockResolvedValue([{ sha: "a", short: "a", selector: "", subject: "Commit" }]);
  rebaseEditor.openWith("base");
  const panel = mount(RebaseTodo, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.querySelector("select")).not.toBeNull());
  const action = target.querySelector<HTMLSelectElement>("select")!;
  action.value = "drop";
  action.dispatchEvent(new Event("change", { bubbles: true }));
  await tick();
  button("Start rebase").click();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't start the interactive rebase", headline: "Interactive rebase: at least one non-drop step is required." });
  expect(rebaseEditor.open).toBe(true);
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/EditFailures.coverage.test.ts`

Expected: FAIL because rewrite has no popup and all validation guards are status-only.

- [ ] **Step 3: Implement precise message replacements**

`src/lib/components/ApplyPanel.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportError, reportNotice } from "../errors.svelte";
```

`src/lib/components/EditTabs.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportNotice } from "../errors.svelte";
```

`src/lib/components/RebaseTodo.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportNotice } from "../errors.svelte";
```

`src/lib/components/ApplyPanel.svelte` — **Before**
```ts
      appState.status =
        "Commit-time editing is unavailable — the bundled git-filter-repo couldn't run.";
```

**After**
```ts
      reportNotice("Can't rewrite history", "Commit-time editing is unavailable — the bundled git-filter-repo couldn't run.");
```

`src/lib/components/ApplyPanel.svelte` — **Before**
```ts
      appState.status = "No new dates queued. Use an edit mode to preview changes first.";
```

**After**
```ts
      reportNotice("Can't rewrite history", "No new dates queued. Use an edit mode to preview changes first.");
```

`src/lib/components/ApplyPanel.svelte` — **Before**
```ts
      appState.status = `Rewrite failed: ${e}`;
      appState.appendLog(`[rewrite] ERROR: ${e}`);
```

**After**
```ts
      appState.appendLog("[rewrite] stopped.");
      reportError("Rewrite failed", e);
```

`src/lib/components/EditTabs.svelte` — **Before**
```ts
      appState.status = "Select one or more commits first.";
```

**After**
```ts
      reportNotice("Can't preview date changes", "Select one or more commits first.");
```

`src/lib/components/EditTabs.svelte` in both `previewExact` and `previewCompress` — **Before**
```ts
      appState.status = "Enter a complete, valid date.";
```

**After**
```ts
      reportNotice("Can't preview date changes", "Enter a complete, valid date.");
```

`src/lib/components/EditTabs.svelte` — **Before**
```ts
      appState.status = "Compress needs at least 2 selected commits.";
```

**After**
```ts
      reportNotice("Can't preview date changes", "Compress needs at least 2 selected commits.");
```

`src/lib/components/EditTabs.svelte` — **Before**
```ts
      appState.status = "Compress: end must be after start.";
```

**After**
```ts
      reportNotice("Can't preview date changes", "Compress: end must be after start.");
```

`src/lib/components/EditTabs.svelte` — **Before**
```ts
      appState.status = "Compress: couldn't read dates for the selected commits.";
```

**After**
```ts
      reportNotice("Can't preview date changes", "Compress: couldn't read dates for the selected commits.");
```

`src/lib/components/RebaseTodo.svelte` — **Before**
```ts
      appState.status = "Interactive rebase: at least one non-drop step is required.";
```

**After**
```ts
      reportNotice("Can't start the interactive rebase", "Interactive rebase: at least one non-drop step is required.");
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/EditFailures.coverage.test.ts src/lib/dates.test.ts`

Expected: PASS; every notice body is unchanged, rewrite dates remain queued on failure, progress logs remain, and raw failure details appear once.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/ApplyPanel.svelte src/lib/components/EditTabs.svelte src/lib/components/RebaseTodo.svelte src/lib/components/EditFailures.coverage.test.ts
git commit -F - <<'COMMIT'
fix: report history editing failures and refusals

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 7: Add shared retryable load errors to stash and reflog

**Files:**
- Create: `src/lib/components/LoadError.svelte`.
- Modify: `src/lib/components/StashPanel.svelte` (`loadStash` and state rendering).
- Modify: `src/lib/components/ReflogPanel.svelte` (`loadReflog` and state rendering).
- Create/Test: `src/lib/components/LoadError.test.ts`.
- Create/Test: `src/lib/components/HistoryLoads.coverage.test.ts`.

**Interfaces:**
- Consumes: `errorText(e: unknown): string`; `summarizeGitOutput(text: string): GitErrorSummary`; `api.stashList(repo: string): Promise<StashEntry[]>`; `api.reflog(repo: string, limit?: number): Promise<ReflogEntry[]>`.
- Produces: new `LoadError` component props `{ what: string; error: unknown; onRetry: () => void }`, rendering `Couldn't load {what}: {headline}`, so `what` is phrased to follow "Couldn't load" ("the stashes", "the reflog"); unchanged component-local `loadStash(): Promise<void>` and `loadReflog(): Promise<void>`.
- Later inline tasks consume `LoadError` exactly as defined here. Each loader owns its `loaded`/`loadError` state and its request sequence; the component only summarizes and renders. A refresh keeps the current list on screen: `loaded` flips to true on the first success and no loader resets it.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/LoadError.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { mount, unmount } from "svelte";
import { errors } from "../errors.svelte";
import LoadError from "./LoadError.svelte";
let dispose: (() => Promise<void>) | undefined;
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  document.body.replaceChildren();
});
it("shows only the first summarized headline line, selectable, with Retry", () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const retry = vi.fn();
  const panel = mount(LoadError, { target, props: { what: "the stashes", error: new Error("From example\nfatal: cannot read\n\tfile.txt\nhint: try later"), onRetry: retry } });
  dispose = () => unmount(panel);
  expect(target.querySelector(".selectable")?.textContent).toBe("Couldn't load the stashes: cannot read");
  expect(target.textContent).not.toContain("file.txt");
  target.querySelector<HTMLButtonElement>("button")!.click();
  expect(retry).toHaveBeenCalledOnce();
  expect(errors.pending).toBe(0);
});
it.each([null, undefined])("renders an empty rejection (%s) as a real failure", (error) => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(LoadError, { target, props: { what: "the diff", error, onRetry: () => {} } });
  dispose = () => unmount(panel);
  expect(target.textContent).toContain("Couldn't load the diff: The operation failed without an error message.");
});
```

Create `src/lib/components/HistoryLoads.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import StashPanel from "./StashPanel.svelte";
import ReflogPanel from "./ReflogPanel.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
vi.mock("svelte/transition", async (original) => ({ ...await original<typeof import("svelte/transition")>(), slide: () => ({ duration: 0 }) }));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/history";
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
// Each row mounts its own concrete component through `open`, so no `mount` call receives a
// union of two component types (their props differ: ReflogPanel has `bare`).
const panels = [
  {
    name: "stashes",
    open: () => mount(StashPanel, { target }),
    method: "stashList" as const,
    what: "the stashes",
    empty: "No stashes.",
  },
  {
    name: "reflog",
    open: () => mount(ReflogPanel, { target }),
    method: "reflog" as const,
    what: "the reflog",
    empty: "No reflog entries.",
  },
];
it.each(panels)("retries $name without showing a fake empty state", async ({ open, method, what, empty }) => {
  const load = vi.spyOn(api, method).mockRejectedValueOnce(new Error("fatal: cannot read")).mockResolvedValue([]);
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(`Couldn't load ${what}: cannot read`));
  expect(target.textContent).not.toContain(empty);
  expect(errors.pending).toBe(0);
  const retry = Array.from(target.querySelectorAll("button")).find((b) => b.textContent === "Retry")!;
  retry.click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  expect(load).toHaveBeenCalledTimes(2);
  expect(target.textContent).not.toContain("Couldn't load");
});
it.each(panels)("keeps pending $name distinct from successful empty", async ({ open, method, empty }) => {
  let finish!: () => void;
  vi.spyOn(api, method).mockImplementation(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await tick();
  expect(target.textContent).not.toContain(empty);
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
});
it.each(panels)("keeps the current $name list on screen while a refresh runs", async ({ open, method, empty }) => {
  let finish!: () => void;
  const load = vi
    .spyOn(api, method)
    .mockResolvedValueOnce([])
    .mockImplementationOnce(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  // A working-copy / undo refresh re-runs the loader; the list must not flash "Loading".
  appState.setWorkingChanges([]);
  appState.setLastUndo({ branch: "main", sha: "a", label: "Reset" });
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  expect(target.textContent).toContain(empty);
  expect(target.textContent).not.toContain("Loading");
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
});
it.each(panels)("ignores an obsolete $name failure after repository switch", async ({ open, method, empty }) => {
  let reject!: (e: unknown) => void;
  const load = vi.spyOn(api, method).mockImplementationOnce(() => new Promise<never[]>((_, fail) => { reject = fail; })).mockResolvedValue([]);
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await tick();
  appState.repo = "/tmp/new-history";
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  reject(new Error("obsolete failure"));
  await tick();
  await Promise.resolve();
  expect(target.textContent).not.toContain("obsolete failure");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/LoadError.test.ts src/lib/components/HistoryLoads.coverage.test.ts`

Expected: FAIL first because `LoadError.svelte` does not exist; the history tests also fail because failed loads render empty copy and have no Retry. (The refresh test passes on the current tree, since a refresh already keeps the list; it is there to keep it that way.)

- [ ] **Step 3: Implement the shared view and loaders**

Create `src/lib/components/LoadError.svelte` with this complete content:

```svelte
<script lang="ts">
  import { errorText, summarizeGitOutput } from "../errorReport";

  let { what, error, onRetry }: { what: string; error: unknown; onRetry: () => void } = $props();
  const headline = $derived(summarizeGitOutput(errorText(error)).headline.split("\n")[0]);
</script>

<div class="load-error" role="status">
  <p class="selectable">Couldn't load {what}: {headline}</p>
  <button type="button" onclick={onRetry}>Retry</button>
</div>

<style>
  /* No outer margin: each panel spaces it like its own notes (StashPanel's `.note` is
     margin 0; ConflictView uses a flex gap; the diff pane wraps it in `.diff-error`). */
  .load-error {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 12px;
    line-height: 1.45;
  }
  p {
    flex: 1 1 180px;
    min-width: 0;
    margin: 0;
    color: var(--danger);
    overflow-wrap: anywhere;
  }
  /* Same recipe as the sibling buttons in StashPanel / ReflogPanel / BackupsPanel. */
  button {
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--btn-bg);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    background: var(--btn-hover);
  }
</style>
```

The compact note typography follows CommitDetail's inline diff pattern; use the requested danger token rather than copying its amber `--err`. Do not modify CommitDetail or global CSS. Every `what` is phrased to follow "Couldn't load".

`src/lib/components/StashPanel.svelte` — **Before**
```svelte
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
```

**After**
```svelte
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import LoadError from "./LoadError.svelte";
```

`src/lib/components/StashPanel.svelte` — **Before**
```svelte
  let entries = $state<StashEntry[]>([]);
```

**After**
```svelte
  let entries = $state<StashEntry[]>([]);
  // Set when the last load failed, so the panel says so instead of "No stashes.".
  // Wrapped so that a null/undefined rejection is still truthy.
  let loadError = $state<{ error: unknown } | null>(null);
  // True once a load has succeeded. Never reset by a refresh: the effect above re-runs on
  // every working-copy refresh, and the current list stays on screen until the new one lands.
  let loaded = $state(false);
  // Bumped per request; a response from a superseded request is dropped.
  let loadSequence = 0;
```

`src/lib/components/StashPanel.svelte` — **Before**
```svelte
  async function loadStash() {
    if (!isTauri() || !appState.repo) return;
    try {
      entries = await api.stashList(appState.repo);
    } catch (e) {
      console.warn("[gte] stash list failed", e);
      entries = [];
    }
  }
```

**After**
```svelte
  async function loadStash() {
    if (!isTauri() || !appState.repo) return;
    const repo = appState.repo;
    const sequence = ++loadSequence;
    try {
      const result = await api.stashList(repo);
      if (sequence !== loadSequence || repo !== appState.repo) return;
      entries = result;
      loaded = true;
      loadError = null;
    } catch (e) {
      if (sequence !== loadSequence || repo !== appState.repo) return;
      loadError = { error: e };
    }
  }
```

`src/lib/components/StashPanel.svelte` — **Before**
```svelte
    {#if entries.length === 0}
      <p class="note">No stashes. Stash changes to save them temporarily.</p>
```

**After**
```svelte
    {#if !appState.repo}
      <p class="note">Open a repository to see stashes.</p>
    {:else if loadError}
      <LoadError what="the stashes" error={loadError.error} onRetry={() => void loadStash()} />
    {:else if !loaded}
      <p class="note">Loading stashes…</p>
    {:else if entries.length === 0}
      <p class="note">No stashes. Stash changes to save them temporarily.</p>
```

`src/lib/components/ReflogPanel.svelte` — **Before**
```svelte
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
```

**After**
```svelte
  import CollapsiblePanel from "./CollapsiblePanel.svelte";
  import LoadError from "./LoadError.svelte";
```

`src/lib/components/ReflogPanel.svelte` — **Before**
```svelte
  let entries = $state<ReflogEntry[]>([]);
```

**After**
```svelte
  let entries = $state<ReflogEntry[]>([]);
  // Set when the last load failed, so the panel says so instead of "No reflog entries.".
  // Wrapped so that a null/undefined rejection is still truthy.
  let loadError = $state<{ error: unknown } | null>(null);
  // True once a load has succeeded. Never reset by a refresh, so the current list stays on
  // screen until the new one lands.
  let loaded = $state(false);
  // Bumped per request; a response from a superseded request is dropped.
  let loadSequence = 0;
```

`src/lib/components/ReflogPanel.svelte` — **Before**
```svelte
  async function loadReflog() {
    if (!isTauri() || !appState.repo) return;
    try {
      entries = await api.reflog(appState.repo, 50);
    } catch (e) {
      console.warn("[gte] reflog load failed", e);
      entries = [];
    }
  }
```

**After**
```svelte
  async function loadReflog() {
    if (!isTauri() || !appState.repo) return;
    const repo = appState.repo;
    const sequence = ++loadSequence;
    try {
      const result = await api.reflog(repo, 50);
      if (sequence !== loadSequence || repo !== appState.repo) return;
      entries = result;
      loaded = true;
      loadError = null;
    } catch (e) {
      if (sequence !== loadSequence || repo !== appState.repo) return;
      loadError = { error: e };
    }
  }
```

`src/lib/components/ReflogPanel.svelte` — **Before**
```svelte
  {:else if entries.length === 0}
    <p class="note">No reflog entries. Open a repository to see history.</p>
```

**After**
```svelte
  {:else if !appState.repo}
    <p class="note">Open a repository to see history.</p>
  {:else if loadError}
    <LoadError what="the reflog" error={loadError.error} onRetry={() => void loadReflog()} />
  {:else if !loaded}
    <p class="note">Loading reflog entries…</p>
  {:else if entries.length === 0}
    <p class="note">No reflog entries. Open a repository to see history.</p>
```

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/LoadError.test.ts src/lib/components/HistoryLoads.coverage.test.ts`

Expected: PASS. Initial pending, failure, retry, and successful empty states are distinct; a refresh keeps the current list on screen; no popup from either list loader.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/LoadError.svelte src/lib/components/StashPanel.svelte src/lib/components/ReflogPanel.svelte src/lib/components/LoadError.test.ts src/lib/components/HistoryLoads.coverage.test.ts
git commit -F - <<'COMMIT'
fix: show retryable stash and reflog load errors

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 8: Explain a conflict list that failed to load

**Files:**
- Modify: `src/lib/components/ConflictView.svelte` (details state, `refreshDetails`, its `$effect`, and the list branch of the template).
- Create/Test: `src/lib/components/ConflictView.coverage.test.ts`.

**Interfaces:**
- Consumes: `api.conflictDetails(repo: string): Promise<ConflictEntry[]>`; `LoadError` props `{ what: string; error: unknown; onRetry: () => void }` (Task 7); `appState.setRepoStatus(s: RepoStatus | null): void`.
- Produces: unchanged local `refreshDetails(): Promise<void>`.

Scope note: resolution ("all conflicts resolved") and the Continue/Amend controls stay driven by git's own conflict count (`conflicted` from `repo_status`), exactly as today. That count is authoritative, and it comes from a separate call that succeeded. The only defect is that a failed *details* load renders an unexplained empty list. So this task replaces that empty list with an inline error plus Retry, and changes nothing else. Gating Continue on the details load would add a dead end (a details failure would block a merge git considers ready to continue) without preventing anything, since git refuses Continue itself while conflicts remain.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/ConflictView.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import ConflictView from "./ConflictView.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
function setStatus(conflicted: number) {
  appState.setRepoStatus({
    head: { sha: "a", branch: "main", detached: false },
    staged: 0,
    unstaged: 0,
    untracked: 0,
    conflicted,
    operation: "merge",
  });
}
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/conflicts";
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.setRepoStatus(null);
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it.each([new Error("fatal: index locked"), null])(
  "shows a retryable load error instead of an empty list (%s)",
  async (error) => {
    setStatus(1);
    const load = vi
      .spyOn(api, "conflictDetails")
      .mockRejectedValueOnce(error)
      .mockResolvedValue([{ path: "f", kind: "both" }]);
    const panel = mount(ConflictView, { target });
    dispose = () => unmount(panel);
    await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details:"));
    // A background load failure is inline, never a popup.
    expect(errors.pending).toBe(0);
    Array.from(target.querySelectorAll("button"))
      .find((b) => b.textContent === "Retry")!
      .click();
    await vi.waitFor(() => expect(target.textContent).toContain("Use ours"));
    expect(target.textContent).not.toContain("Couldn't load the conflict details:");
    expect(load).toHaveBeenCalledTimes(2);
  },
);
it("leaves Continue to git's conflict count when the details fail", async () => {
  setStatus(0);
  vi.spyOn(api, "conflictDetails").mockRejectedValue(new Error("fatal: index locked"));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details:"));
  expect(target.querySelector<HTMLButtonElement>(".primary")!.disabled).toBe(false);
});
it("discards a failed request after the operation ends", async () => {
  setStatus(1);
  let reject!: (e: unknown) => void;
  vi.spyOn(api, "conflictDetails").mockImplementation(
    () => new Promise((_, fail) => {
      reject = fail;
    }),
  );
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await tick();
  appState.setRepoStatus(null);
  await tick();
  reject(new Error("obsolete"));
  await tick();
  expect(target.querySelector("section")).toBeNull();
  expect(errors.pending).toBe(0);
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/ConflictView.coverage.test.ts`

Expected: FAIL. The `it.each` cases time out waiting for "Couldn't load the conflict details:", because the panel swallows the failure and renders an empty list.

- [ ] **Step 3: Implement the inline load error**

`src/lib/components/ConflictView.svelte`, **Before**:
```svelte
  import type { ConflictEntry } from "../types";
```

**After**:
```svelte
  import type { ConflictEntry } from "../types";
  import LoadError from "./LoadError.svelte";
```

`src/lib/components/ConflictView.svelte`, **Before**:
```svelte
  let details = $state<ConflictEntry[]>([]);
```

**After**:
```svelte
  let details = $state<ConflictEntry[]>([]);
  // Set when the file list failed to load, so the panel says so instead of listing nothing.
  // Wrapped so that a null/undefined rejection is still truthy.
  let detailsError = $state<{ error: unknown } | null>(null);
  // Bumped per request; a response from a superseded request (or an ended op) is dropped.
  let detailsSequence = 0;
```

`src/lib/components/ConflictView.svelte`, **Before**:
```svelte
  async function refreshDetails() {
    if (!inTauri() || !appState.repo || !op) {
      details = [];
      return;
    }
    try {
      details = await api.conflictDetails(appState.repo);
    } catch {
      details = [];
    }
  }

  // Re-fetch the file list whenever an op is present and the conflict count changes
  // (each resolve decrements it). Reading both signals registers them as deps.
  $effect(() => {
    const o = appState.repoStatus?.operation;
    void appState.repoStatus?.conflicted;
    if (o) refreshDetails();
    else details = [];
  });
```

**After**:
```svelte
  async function refreshDetails() {
    const sequence = ++detailsSequence;
    detailsError = null;
    if (!inTauri() || !appState.repo || !op) {
      details = [];
      return;
    }
    const repo = appState.repo;
    try {
      const result = await api.conflictDetails(repo);
      if (sequence !== detailsSequence || repo !== appState.repo) return;
      details = result;
    } catch (e) {
      if (sequence !== detailsSequence || repo !== appState.repo) return;
      details = [];
      detailsError = { error: e };
    }
  }

  // Re-fetch the file list whenever an op is present and the conflict count changes
  // (each resolve decrements it). Reading both signals registers them as deps.
  $effect(() => {
    const o = appState.repoStatus?.operation;
    void appState.repoStatus?.conflicted;
    if (o) refreshDetails();
    else {
      detailsSequence++;
      details = [];
      detailsError = null;
    }
  });
```

`src/lib/components/ConflictView.svelte`, **Before**:
```svelte
    {#if details.length}
```

**After**:
```svelte
    {#if detailsError}
      <LoadError what="the conflict details" error={detailsError.error} onRetry={() => void refreshDetails()} />
    {:else if details.length}
```

Leave `class:resolved={conflicted === 0}`, the "all conflicts resolved" header text, the `{:else if conflicted === 0}` branch, and the Continue button's `disabled={conflicted > 0}` unchanged.

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/ConflictView.coverage.test.ts src/lib/components/LoadError.test.ts`

Expected: PASS.

Run: `npm run check`
Expected: 0 errors and 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/ConflictView.svelte src/lib/components/ConflictView.coverage.test.ts
git commit -F - <<'COMMIT'
fix: explain a conflict list that failed to load

A failed conflict-details load rendered an empty list with no reason.
Show the error with Retry instead; resolution and Continue still follow
git's own conflict count.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 9: Render retryable working-copy diff errors

**Files:**
- Modify: `src/lib/components/WorkingCopyView.svelte` (diff state/effect/rendering and one style rule).
- Create/Test: `src/lib/components/WorkingCopyView.load.test.ts`.

**Interfaces:**
- Consumes: `api.diff(repo: string, path: string | null, staged: boolean, untracked?: boolean, context?: number): Promise<string>`; `LoadError` props `{ what: string; error: unknown; onRetry: () => void }`.
- Produces: unchanged `WorkingCopyView` public interface; Retry increments a local runes counter and re-runs the existing diff effect. The effect's cleanup rejects stale responses, including same-path/same-key requests from another repository.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/WorkingCopyView.load.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import WorkingCopyView from "./WorkingCopyView.svelte";
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn(), revealItemInDir: vi.fn() }));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
const file = { path: "f.txt", status: "modified", staged: false, unstaged: true, untracked: false, conflicted: false };
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  // DiffView reads window.matchMedia at init and jsdom has none; it mounts once a diff loads.
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  appState.repo = "/tmp/diff";
  appState.setWorkingChanges([file]);
  appState.setSelectedFile({ path: "f.txt", section: "unstaged" });
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("shows the summarized failure with Retry, then permits a successful empty diff", async () => {
  const load = vi.spyOn(api, "diff").mockRejectedValueOnce(new Error("From origin\nfatal: cannot read file")).mockResolvedValue("");
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the diff: cannot read file"));
  expect(target.querySelector(".diff-pane")!.textContent).not.toContain("No changes.");
  expect(errors.pending).toBe(0);
  target.querySelector<HTMLButtonElement>(".load-error button")!.click();
  await vi.waitFor(() => expect(target.querySelector(".diff-pane")!.textContent).toContain("No changes."));
  expect(load).toHaveBeenCalledTimes(2);
  expect(load).toHaveBeenLastCalledWith("/tmp/diff", "f.txt", false, false, appState.effectiveDiffContext);
});
it("does not show an empty diff while a request is pending", async () => {
  let finish!: () => void;
  vi.spyOn(api, "diff").mockImplementation(() => new Promise((resolve) => { finish = () => resolve(""); }));
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  expect(target.querySelector(".diff-pane")!.textContent).toContain("Loading diff…");
  expect(target.querySelector(".diff-pane")!.textContent).not.toContain("No changes.");
  finish();
  await vi.waitFor(() => expect(target.querySelector(".diff-pane")!.textContent).toContain("No changes."));
});
it("ignores another repository's failure for the same selected path", async () => {
  let reject!: (e: unknown) => void;
  const load = vi.spyOn(api, "diff").mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; })).mockResolvedValue("");
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  appState.repo = "/tmp/other-diff";
  appState.setWorkingChanges([file]);
  appState.setSelectedFile({ path: "f.txt", section: "unstaged" });
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.querySelector(".diff-pane")!.textContent).toContain("No changes."));
  reject(new Error("obsolete error"));
  await tick();
  await Promise.resolve();
  expect(target.textContent).not.toContain("obsolete error");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/WorkingCopyView.load.test.ts`

Expected: FAIL because a rejected diff is replaced with an empty patch and has no Retry.

- [ ] **Step 3: Implement diff state and the shared error view**

`src/lib/components/WorkingCopyView.svelte` — **Before**
```svelte
  import DiffView from "./DiffView.svelte";
```

**After**
```svelte
  import DiffView from "./DiffView.svelte";
  import LoadError from "./LoadError.svelte";
```

`src/lib/components/WorkingCopyView.svelte` — **Before**
```svelte
  let diffPatch = $state("");
  let diffLoading = $state(false);

  $effect(() => {
    const key = diffKey; // register dependency
    if (!key || !selectedFile || !isTauri()) {
      diffPatch = "";
      return;
    }
    diffLoading = true;
    api
      .diff(appState.repo, selectedFile, selectedIsStaged, selectedIsUntracked, appState.effectiveDiffContext)
      .then((p) => {
        // Guard stale results: only apply if the key hasn't changed.
        if (key === diffKey) {
          diffPatch = p;
        }
      })
      .catch(() => {
        if (key === diffKey) diffPatch = "";
      })
      .finally(() => {
        if (key === diffKey) diffLoading = false;
      });
  });

```

**After**
```svelte
  let diffPatch = $state("");
  // Pending is `!diffLoaded && !diffError`: every key change starts a fresh load, as today.
  let diffLoaded = $state(false);
  // Wrapped so that a null/undefined rejection is still truthy.
  let diffError = $state<{ error: unknown } | null>(null);
  // Bumped by Retry so the effect re-runs for the same key.
  let diffRetry = $state(0);

  $effect(() => {
    const key = diffKey;
    const repo = appState.repo;
    void diffRetry;
    diffError = null;
    diffLoaded = false;
    if (!key || !selectedFile || !isTauri()) {
      diffPatch = "";
      return;
    }
    // Cleared by the cleanup below, so a response for a superseded key (or another
    // repository holding the same path) never lands.
    let active = true;
    api
      .diff(repo, selectedFile, selectedIsStaged, selectedIsUntracked, appState.effectiveDiffContext)
      .then((p) => {
        if (!active) return;
        diffPatch = p;
        diffLoaded = true;
      })
      .catch((e) => {
        if (!active) return;
        diffPatch = "";
        diffError = { error: e };
      });
    return () => {
      active = false;
    };
  });

```

`src/lib/components/WorkingCopyView.svelte` — **Before**
```svelte
        {:else if diffLoading}
          <p class="diff-loading">Loading diff…</p>
        {:else}
          <DiffView
```

**After**
```svelte
        {:else if diffError}
          <div class="diff-error">
            <LoadError what="the diff" error={diffError.error} onRetry={() => { diffRetry += 1; }} />
          </div>
        {:else if !diffLoaded}
          <p class="diff-loading">Loading diff…</p>
        {:else}
          <DiffView
```

No browser-preview branch is added here: the whole view is already replaced outside Tauri (`{#if !isTauri()}` at the top of the template), so the diff pane never renders there.

`src/lib/components/WorkingCopyView.svelte` `<style>` — **Before**
```svelte
  .diff-loading {
    margin: 0;
    padding: 12px 14px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
```

**After**
```svelte
  .diff-loading {
    margin: 0;
    padding: 12px 14px;
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }
  /* Same inset as .diff-loading, so the inline error lines up with the pane's other notes. */
  .diff-error {
    padding: 12px 14px;
  }
```

Leave Task 5's native callbacks intact. Do not change DiffView's Shiki fallback or CommitDetail's existing diff loader.

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/WorkingCopyView.load.test.ts src/lib/components/NativeActions.coverage.test.ts src/lib/workingSection.test.ts`

Expected: PASS; Retry loads the same current file/context, stale failures cannot overwrite it, and `diffLoading` no longer exists in the file (`grep -n diffLoading src/lib/components/WorkingCopyView.svelte` prints nothing).

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/WorkingCopyView.svelte src/lib/components/WorkingCopyView.load.test.ts
git commit -F - <<'COMMIT'
fix: show retryable working copy diff failures

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 10: Report backup actions and separate backup-list failures

**Files:**
- Modify: `src/lib/components/BackupsPanel.svelte`.
- Create/Test: `src/lib/components/BackupsPanel.coverage.test.ts`.

**Interfaces:**
- Consumes: `api.listBundles(repo: string): Promise<BundleInfo[]>`; `api.listSafetyRefs(repo: string): Promise<SafetyRef[]>`; `api.createBundle(repo: string): Promise<string>`; `api.deleteBundle(path: string): Promise<void>`; `api.fetchBundle(repo: string, bundlePath: string): Promise<string>`; `api.deleteRefs(repo: string, refs: string[]): Promise<void>`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`; `LoadError` props `{ what: string; error: unknown; onRetry: () => void }`.
- Produces: unchanged `BackupsPanel` props `{ bare?: boolean }` and local async signatures `refreshBundles(): Promise<void>`, `refreshSafetyRefs(): Promise<void>`, `createBundleNow(): Promise<void>`, `deleteBundleNow(): Promise<void>`, `fetchBundleNow(): Promise<void>`, `deleteSelectedRefs(): Promise<void>`, `deleteAllRefs(): Promise<void>`.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/BackupsPanel.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import BackupsPanel from "./BackupsPanel.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let target: HTMLDivElement;
let dispose: (() => Promise<void>) | undefined;
function button(text: string): HTMLButtonElement {
  const found = Array.from(target.querySelectorAll("button")).find((b) => b.textContent?.trim() === text);
  if (!found) throw new Error(`Missing button: ${text}`);
  return found;
}
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/backups";
  vi.spyOn(api, "listBundles").mockResolvedValue([{ name: "backup.bundle", path: "/tmp/backup.bundle", size: 12 }]);
  vi.spyOn(api, "listSafetyRefs").mockResolvedValue([{ kind: "original", name: "refs/original/main" }]);
  vi.stubGlobal("confirm", vi.fn(() => true));
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it.each([
  ["bundles", "Couldn't load the bundles:", "No bundles yet."],
  ["safety", "Couldn't load the safety refs:", "No safety refs"],
])("keeps failed %s lists inline and retries to successful empty", async (kind, prefix, empty) => {
  if (kind === "bundles") vi.mocked(api.listBundles).mockRejectedValueOnce(new Error("fatal: denied")).mockResolvedValue([]);
  else vi.mocked(api.listSafetyRefs).mockRejectedValueOnce(new Error("fatal: denied")).mockResolvedValue([]);
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  if (kind === "safety") button("Safety refs").click();
  await vi.waitFor(() => expect(target.textContent).toContain(`${prefix} denied`));
  expect(target.textContent).not.toContain(empty);
  expect(errors.pending).toBe(0);
  button("Retry").click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  expect(target.textContent).not.toContain(prefix);
});
it.each([
  ["create", "Create bundle now", "Couldn't create the bundle"],
  ["delete", "Delete", "Couldn't delete the bundle"],
  ["fetch", "Fetch refs", "Couldn't fetch from the bundle"],
  ["selected", "Delete selected", "Couldn't delete the safety refs"],
  ["all", "Delete all", "Couldn't delete all safety refs"],
])("reports %s action once", async (kind, label, title) => {
  const failure = new Error("fatal: operation denied");
  vi.spyOn(api, "createBundle").mockRejectedValue(failure);
  vi.spyOn(api, "deleteBundle").mockRejectedValue(failure);
  vi.spyOn(api, "fetchBundle").mockRejectedValue(failure);
  vi.spyOn(api, "deleteRefs").mockRejectedValue(failure);
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.querySelector(".row-btn")).not.toBeNull());
  if (kind === "selected" || kind === "all") {
    button("Safety refs").click();
    await vi.waitFor(() => expect(target.textContent).toContain("refs/original/main"));
  }
  if (kind !== "create") {
    target.querySelector<HTMLButtonElement>(".row-btn")!.click();
    await tick();
  }
  button(label).click();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title, headline: "operation denied" }));
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
});
it("keeps a post-create list failure inline rather than reporting creation twice", async () => {
  vi.spyOn(api, "createBundle").mockResolvedValue("/tmp/new.bundle");
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.querySelector(".row-btn")).not.toBeNull());
  vi.mocked(api.listBundles).mockRejectedValue(new Error("fatal: list unavailable"));
  button("Create bundle now").click();
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the bundles: list unavailable"));
  expect(errors.pending).toBe(0);
});
it("keeps the current bundle list on screen while a refresh runs", async () => {
  let finish!: () => void;
  vi.mocked(api.listBundles)
    .mockResolvedValueOnce([{ name: "backup.bundle", path: "/tmp/backup.bundle", size: 12 }])
    .mockImplementationOnce(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("backup.bundle"));
  button("Refresh").click();
  await tick();
  expect(target.textContent).toContain("backup.bundle");
  expect(target.textContent).not.toContain("Loading bundles");
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain("No bundles yet."));
});
it("loads the active safety list again after a repository switch", async () => {
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  button("Safety refs").click();
  await vi.waitFor(() => expect(target.textContent).toContain("refs/original/main"));
  vi.mocked(api.listSafetyRefs).mockResolvedValue([]);
  appState.repo = "/tmp/other-backups";
  await vi.waitFor(() => expect(api.listSafetyRefs).toHaveBeenLastCalledWith("/tmp/other-backups"));
  await vi.waitFor(() => expect(target.textContent).toContain("No safety refs"));
  expect(target.textContent).not.toContain("refs/original/main");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/BackupsPanel.coverage.test.ts`

Expected: FAIL because list failures are status-only, action failures do not queue errors, and the active safety list is not refreshed on repository change. (The refresh test passes on the current tree, since a refresh already keeps the list; it is there to keep it that way.)

- [ ] **Step 3: Implement independent list state and action reports**

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
  import { appState } from "../store.svelte";
```

**After**
```svelte
  import { untrack } from "svelte";
  import { appState } from "../store.svelte";
  import { reportError } from "../errors.svelte";
  import LoadError from "./LoadError.svelte";
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
  let safetyRefs = $state<SafetyRef[]>([]);
```

**After**
```svelte
  let safetyRefs = $state<SafetyRef[]>([]);
  // Set when a list's last load failed, so the panel says so instead of "No bundles yet.".
  // Wrapped so that a null/undefined rejection is still truthy.
  let bundlesError = $state<{ error: unknown } | null>(null);
  let safetyError = $state<{ error: unknown } | null>(null);
  // True once a list has loaded for this repository. A refresh keeps the current list on
  // screen; only the repo-switch effect below resets these.
  let bundlesLoaded = $state(false);
  let safetyLoaded = $state(false);
  // Bumped per request; a response from a superseded request is dropped.
  let bundlesSequence = 0;
  let safetySequence = 0;
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
  // Refresh bundles whenever the repo changes — only in Tauri.
  $effect(() => {
    if (isTauri() && appState.repo) refreshBundles();
  });
```

**After**
```svelte
  // Reset selections on repo switch — `selectedBundle` is a bare path and `deleteRefs` takes
  // names, so a selection carried over would delete the PREVIOUS repo's bundle or refs —
  // and refresh the active safety list too.
  $effect(() => {
    const repo = appState.repo;
    selectedBundle = null;
    selectedRefs = new Set();
    bundles = [];
    safetyRefs = [];
    bundlesLoaded = false;
    safetyLoaded = false;
    bundlesError = null;
    safetyError = null;
    if (isTauri() && repo) {
      void refreshBundles();
      if (untrack(() => tab) === "safety") void refreshSafetyRefs();
    }
  });
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
  async function refreshBundles() {
    if (!isTauri() || !appState.repo) return;
    try {
      bundles = await api.listBundles(appState.repo);
    } catch (e) {
      appState.status = `List bundles failed: ${e}`;
    }
  }
```

**After**
```svelte
  async function refreshBundles() {
    if (!isTauri() || !appState.repo) return;
    const repo = appState.repo;
    const sequence = ++bundlesSequence;
    try {
      const result = await api.listBundles(repo);
      if (sequence !== bundlesSequence || repo !== appState.repo) return;
      bundles = result;
      bundlesLoaded = true;
      bundlesError = null;
    } catch (e) {
      if (sequence !== bundlesSequence || repo !== appState.repo) return;
      bundlesError = { error: e };
    }
  }
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
  async function refreshSafetyRefs() {
    if (!isTauri() || !appState.repo) return;
    try {
      safetyRefs = await api.listSafetyRefs(appState.repo);
    } catch (e) {
      appState.status = `List safety refs failed: ${e}`;
    }
  }
```

**After**
```svelte
  async function refreshSafetyRefs() {
    if (!isTauri() || !appState.repo) return;
    const repo = appState.repo;
    const sequence = ++safetySequence;
    try {
      const result = await api.listSafetyRefs(repo);
      if (sequence !== safetySequence || repo !== appState.repo) return;
      safetyRefs = result;
      safetyLoaded = true;
      safetyError = null;
    } catch (e) {
      if (sequence !== safetySequence || repo !== appState.repo) return;
      safetyError = { error: e };
    }
  }
```

`src/lib/components/BackupsPanel.svelte` `createBundleNow` — **Before**
```svelte
      appState.status = `Bundle failed: ${e}`;
```

**After**
```svelte
      reportError("Couldn't create the bundle", e);
```

`src/lib/components/BackupsPanel.svelte` `deleteBundleNow` — **Before**
```svelte
      appState.status = `Delete bundle failed: ${e}`;
```

**After**
```svelte
      reportError("Couldn't delete the bundle", e);
```

`src/lib/components/BackupsPanel.svelte` `fetchBundleNow` — **Before**
```svelte
      appState.status = `Fetch bundle failed: ${e}`;
```

**After**
```svelte
      reportError("Couldn't fetch from the bundle", e);
```

`src/lib/components/BackupsPanel.svelte` `deleteSelectedRefs` only — **Before**
```svelte
      appState.status = `Delete refs failed: ${e}`;
```

**After**
```svelte
      reportError("Couldn't delete the safety refs", e);
```

`src/lib/components/BackupsPanel.svelte` `deleteAllRefs` only — **Before**
```svelte
      appState.status = `Delete refs failed: ${e}`;
```

**After**
```svelte
      reportError("Couldn't delete all safety refs", e);
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
    <p class="note">Desktop app only — backups are not available in the browser preview.</p>
  {:else}
```

**After**
```svelte
    <p class="note">Desktop app only — backups are not available in the browser preview.</p>
  {:else if !appState.repo}
    <p class="note">Open a repository to see backups.</p>
  {:else}
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
    <ul class="list">
      {#each bundles as b}
        <li class:selected={selectedBundle === b.path}>
          <button type="button" class="row-btn" onclick={() => (selectedBundle = b.path)}>
            <span class="mono">{b.name}</span>
            <span class="size">{humanSize(b.size)}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No bundles yet. Click "Create bundle now".</li>
      {/each}
    </ul>
```

**After**
```svelte
    {#if bundlesError}
      <LoadError what="the bundles" error={bundlesError.error} onRetry={() => void refreshBundles()} />
    {:else if !bundlesLoaded}
      <p class="note">Loading bundles…</p>
    {:else}
    <ul class="list">
      {#each bundles as b}
        <li class:selected={selectedBundle === b.path}>
          <button type="button" class="row-btn" onclick={() => (selectedBundle = b.path)}>
            <span class="mono">{b.name}</span>
            <span class="size">{humanSize(b.size)}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No bundles yet. Click "Create bundle now".</li>
      {/each}
    </ul>
    {/if}
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
    <ul class="list">
      {#each safetyRefs as r}
        <li class:selected={selectedRefs.has(r.name)}>
          <button type="button" class="row-btn" onclick={() => toggleRef(r.name)}>
            <span class="kind">{r.kind}</span>
            <span class="mono">{r.name}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No safety refs from past rewrites.</li>
      {/each}
    </ul>
```

**After**
```svelte
    {#if safetyError}
      <LoadError what="the safety refs" error={safetyError.error} onRetry={() => void refreshSafetyRefs()} />
    {:else if !safetyLoaded}
      <p class="note">Loading safety refs…</p>
    {:else}
    <ul class="list">
      {#each safetyRefs as r}
        <li class:selected={selectedRefs.has(r.name)}>
          <button type="button" class="row-btn" onclick={() => toggleRef(r.name)}>
            <span class="kind">{r.kind}</span>
            <span class="mono">{r.name}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No safety refs from past rewrites.</li>
      {/each}
    </ul>
    {/if}
```

`src/lib/components/BackupsPanel.svelte` for both Fetch refs and Delete buttons — **Before**
```svelte
disabled={!selectedBundle}
```

**After**
```svelte
disabled={!selectedBundle || bundlesError !== null}
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
disabled={selectedRefs.size === 0}
```

**After**
```svelte
disabled={selectedRefs.size === 0 || safetyError !== null}
```

`src/lib/components/BackupsPanel.svelte` — **Before**
```svelte
disabled={safetyRefs.length === 0}
```

**After**
```svelte
disabled={safetyRefs.length === 0 || safetyError !== null}
```

The initial bundle `$effect` calls the same inline loader as Refresh/Retry. A list-refresh failure after a successful mutation stays inline; it is not relabeled as a failed mutation. A refresh keeps the current list on screen (`bundlesLoaded`/`safetyLoaded` are reset only by the repo-switch effect). Keep existing confirmation prompts and successful `[bundle]`/`[fetch-bundle]` logs.

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/BackupsPanel.coverage.test.ts src/lib/components/LoadError.test.ts`

Expected: PASS; five mutation failure paths report once, both list failures are retryable inline, a refresh keeps the current list, and a successful empty list alone uses empty-state copy.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/BackupsPanel.svelte src/lib/components/BackupsPanel.coverage.test.ts
git commit -F - <<'COMMIT'
fix: report backup actions and retry failed backup lists

Bundle and safety-ref actions open the error popup; a failed list shows
the error inline with Retry instead of a status line and an empty list.

Selections now reset on a repository switch. The bundle selection is a
bare path and ref deletion takes names, so a selection carried across
the switch could delete the previous repository's bundle or refs.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 11: Preserve GitHub availability errors and wire Retry

**Files:**
- Modify: `src/lib/githubState.svelte.ts` (`ensure`, availability state/getter only).
- Modify: `src/lib/components/github/GithubSetupCard.svelte`.
- Modify: `src/lib/components/github/GithubView.svelte` (setup-card branch only).
- Create/Test: `src/lib/components/github/GithubAvailability.coverage.test.ts`.

**Interfaces:**
- Consumes: `api.githubAvailability(repo: string): Promise<GhAvailability>`; `errorText(e: unknown): string`; `LoadError` props `{ what: string; error: unknown; onRetry: () => void }`.
- Produces: additive `githubState.availabilityError: string | null` getter; unchanged `githubState.ensure(repo: string): Promise<void>` and `githubState.refresh(repo: string): Promise<void> | undefined`.
- Produces: `GithubSetupCard` props `{ availability: GhAvailability | null; availabilityError?: string | null; onretry: () => void }`. Preserve the existing lowercase `onretry` name; adapt it to LoadError's `onRetry` prop.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/github/GithubAvailability.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../../api";
import { appState } from "../../store.svelte";
import { errors } from "../../errors.svelte";
import { githubState } from "../../githubState.svelte";
import type { GhAvailability } from "../../types";
import GithubView from "./GithubView.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let sequence = 0;
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = `/tmp/github-availability-${++sequence}`;
  appState.setActiveView("github");
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("shows the real failure through GithubView and retries without installation advice", async () => {
  const load = vi.spyOn(api, "githubAvailability").mockRejectedValueOnce({ kind: "Other", message: "fatal: gh could not start" }).mockResolvedValue({ kind: "NotAuthed" });
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the GitHub screen: gh could not start"));
  expect(githubState.availability).toBeNull();
  expect(githubState.availabilityError).toBe("fatal: gh could not start");
  expect(target.textContent).not.toContain("brew install gh");
  expect(target.querySelectorAll(".load-error button")).toHaveLength(1);
  expect(errors.pending).toBe(0);
  target.querySelector<HTMLButtonElement>(".load-error button")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain("Sign in to GitHub"));
  expect(githubState.availabilityError).toBeNull();
  expect(load).toHaveBeenCalledTimes(2);
});
it.each([
  [{ kind: "NotInstalled" }, "brew install gh"],
  [{ kind: "NotAuthed" }, "gh auth login"],
  [{ kind: "NoRemote" }, "No GitHub remote"],
] satisfies Array<[GhAvailability, string]>)("preserves true availability result %j", async (result, text) => {
  vi.spyOn(api, "githubAvailability").mockResolvedValue(result);
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain(text));
  expect(githubState.availabilityError).toBeNull();
  expect(target.textContent).not.toContain("Couldn't load");
});
it("keeps null rejection as an error and does not enter an automatic retry loop", async () => {
  const load = vi.spyOn(api, "githubAvailability").mockRejectedValue(null);
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the GitHub screen: The operation failed without an error message."));
  await tick();
  expect(load).toHaveBeenCalledOnce();
  expect(githubState.availabilityError).toBe("");
});
it("discards a failed probe for a previous repository", async () => {
  let reject!: (e: unknown) => void;
  const load = vi.spyOn(api, "githubAvailability").mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; })).mockResolvedValue({ kind: "NoRemote" });
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await tick();
  appState.repo = `/tmp/github-availability-${++sequence}`;
  appState.setActiveView("github");
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.textContent).toContain("No GitHub remote"));
  reject(new Error("obsolete probe"));
  await tick();
  await Promise.resolve();
  expect(githubState.availabilityError).toBeNull();
  expect(target.textContent).not.toContain("obsolete probe");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/components/github/GithubAvailability.coverage.test.ts`

Expected: FAIL because arbitrary probe failures become `NotInstalled`, `availabilityError` is absent, and the view never renders a real probe error.

- [ ] **Step 3: Implement availability-only error state and wiring**

`src/lib/githubState.svelte.ts` — **Before**
```ts
import { api } from "./api";
```

**After**
```ts
import { api } from "./api";
import { errorText } from "./errorReport";
```

`src/lib/githubState.svelte.ts` — **Before**
```ts
  let availability = $state<GhAvailability | null>(null);
```

**After**
```ts
  let availability = $state<GhAvailability | null>(null);
  let availabilityError = $state<string | null>(null);
  let availabilitySequence = 0;
```

`src/lib/githubState.svelte.ts` — **Before**
```ts
  async function ensure(repo: string) {
    if (!isTauri()) return; // gh paths are desktop-only
    if (loadedRepo === repo && availability) return;
    loadedRepo = repo;
    pulls.reset();
    issues.reset();
    releases.reset();
    runs.reset();
    traffic.reset();
    contributors.reset();
    activity.reset();
    milestones.reset();
    labels.reset();
    selectedItem = null;
    readme.reset();
    prDetail.reset();
    prDiff.reset();
    issueDetail.reset();
    availability = null;
    stats = null;
    statsError = null;
    availLoading = true;
    try {
      availability = await api.githubAvailability(repo);
    } catch {
      availability = { kind: "NotInstalled" };
    } finally {
      availLoading = false;
    }
    if (availability && availability.kind === "Ok") {
      void loadStats(repo);
      if (login === null) {
        // Repo-independent; best-effort — the UI just hides own-comment
        // affordances until (unless) it resolves.
        api.githubCurrentLogin().then((l) => (login = l)).catch(() => {});
      }
    }
  }

```

**After**
```ts
  async function ensure(repo: string) {
    if (!isTauri()) return; // gh paths are desktop-only
    if (loadedRepo === repo && (availability || availLoading || availabilityError !== null)) return;
    const sequence = ++availabilitySequence;
    loadedRepo = repo;
    pulls.reset();
    issues.reset();
    releases.reset();
    runs.reset();
    traffic.reset();
    contributors.reset();
    activity.reset();
    milestones.reset();
    labels.reset();
    selectedItem = null;
    readme.reset();
    prDetail.reset();
    prDiff.reset();
    issueDetail.reset();
    availability = null;
    availabilityError = null;
    stats = null;
    statsError = null;
    availLoading = true;
    try {
      const result = await api.githubAvailability(repo);
      if (sequence !== availabilitySequence || loadedRepo !== repo) return;
      availability = result;
    } catch (e) {
      if (sequence !== availabilitySequence || loadedRepo !== repo) return;
      availabilityError = errorText(e);
    } finally {
      if (sequence === availabilitySequence && loadedRepo === repo) availLoading = false;
    }
    if (availability && availability.kind === "Ok") {
      void loadStats(repo);
      if (login === null) {
        // Repo-independent; best-effort — the UI just hides own-comment
        // affordances until (unless) it resolves.
        api.githubCurrentLogin().then((l) => (login = l)).catch(() => {});
      }
    }
  }

```

`src/lib/githubState.svelte.ts` — **Before**
```ts
    get availability() {
      return availability;
    },
```

**After**
```ts
    get availability() {
      return availability;
    },
    get availabilityError() {
      return availabilityError;
    },
```

`src/lib/components/github/GithubSetupCard.svelte` — **Before**
```svelte
<script lang="ts">
  import type { GhAvailability } from "../../types";
  let { availability, onretry }: { availability: GhAvailability; onretry: () => void } = $props();
</script>

<div class="setup">
  {#if availability.kind === "NotInstalled"}
    <h2>GitHub CLI not found</h2>
    <p>The GitHub screen uses the <code>gh</code> command-line tool. Install it, then retry.</p>
    <pre>brew install gh</pre>
    <a href="https://cli.github.com" target="_blank" rel="noreferrer">cli.github.com ↗</a>
  {:else if availability.kind === "NotAuthed"}
    <h2>Sign in to GitHub</h2>
    <p>Authenticate the <code>gh</code> CLI once in your terminal, then retry.</p>
    <pre>gh auth login</pre>
  {:else if availability.kind === "NoRemote"}
    <h2>No GitHub remote</h2>
    <p>This repository has no <code>github.com</code> remote, so there's nothing to show here.</p>
  {/if}
  <button class="retry" type="button" onclick={onretry}>Retry</button>
</div>
```

**After**
```svelte
<script lang="ts">
  import type { GhAvailability } from "../../types";
  import LoadError from "../LoadError.svelte";
  let { availability, availabilityError = null, onretry }: {
    availability: GhAvailability | null;
    availabilityError?: string | null;
    onretry: () => void;
  } = $props();
</script>

<div class="setup">
  {#if availabilityError !== null}
    <LoadError what="the GitHub screen" error={availabilityError} onRetry={onretry} />
  {:else}
    {#if availability?.kind === "NotInstalled"}
      <h2>GitHub CLI not found</h2>
      <p>The GitHub screen uses the <code>gh</code> command-line tool. Install it, then retry.</p>
      <pre>brew install gh</pre>
      <a href="https://cli.github.com" target="_blank" rel="noreferrer">cli.github.com ↗</a>
    {:else if availability?.kind === "NotAuthed"}
      <h2>Sign in to GitHub</h2>
      <p>Authenticate the <code>gh</code> CLI once in your terminal, then retry.</p>
      <pre>gh auth login</pre>
    {:else if availability?.kind === "NoRemote"}
      <h2>No GitHub remote</h2>
      <p>This repository has no <code>github.com</code> remote, so there's nothing to show here.</p>
    {/if}
    <button class="retry" type="button" onclick={onretry}>Retry</button>
  {/if}
</div>
```

`src/lib/components/github/GithubView.svelte` — **Before**
```svelte
  {:else if avail}
    <GithubSetupCard availability={avail} onretry={() => appState.repo && githubState.refresh(appState.repo)} />
```

**After**
```svelte
  {:else if avail || githubState.availabilityError !== null}
    <GithubSetupCard
      availability={avail}
      availabilityError={githubState.availabilityError}
      onretry={() => { if (appState.repo) void githubState.refresh(appState.repo); }}
    />
```

`refresh` already chooses a full `ensure` when availability is null. No new retry method is needed. The guard also remembers a failed probe until explicit Retry and avoids reactive retry loops. Leave `softRefresh`, stats, list-panel caches, and GitHub write actions untouched.

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/components/github/GithubAvailability.coverage.test.ts src/lib/components/github/Skeleton.test.ts src/lib/github/remote.test.ts`

Expected: PASS, including the actual GithubView → setup card → Retry path and all three genuine setup states.

- [ ] **Step 5: Commit**

```bash
git add src/lib/githubState.svelte.ts src/lib/components/github/GithubSetupCard.svelte src/lib/components/github/GithubView.svelte src/lib/components/github/GithubAvailability.coverage.test.ts
git commit -F - <<'COMMIT'
fix: preserve github availability errors with retry

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 12: Distinguish manual and quiet automatic updater failures

**Files:**
- Modify: `src/lib/updater.svelte.ts` (`checkForUpdates`, `presentUpdate`, `reportUpdaterError`).
- Create/Test: `src/lib/updater.coverage.test.ts`.

**Interfaces:**
- Consumes: `api.checkUpdateOnChannel(channel: "stable" | "beta"): Promise<UpdateInfo | null>`; `api.installPendingUpdate(onEvent: (e: DownloadEvent) => void): Promise<void>`; process plugin `relaunch(): Promise<void>`; `errorText(e: unknown): string`; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`; `reportNotice(title: string, message: string): void`.
- Produces: unchanged `manualCheckForUpdates(): Promise<void>`, `startupUpdateCheck(): Promise<void>`, `updateCheckOnActivate(): Promise<void>`, and local `checkForUpdates(source: "manual" | "startup" | "background" | "activation"): Promise<void>`.
- Produces: updated private `reportUpdaterError(title: string, err: unknown, opts?: { quiet?: boolean }): void`. No caller changes in Settings or the macOS menu: both already call `manualCheckForUpdates`.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/updater.coverage.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ check: vi.fn(), install: vi.fn(), confirm: vi.fn(), relaunch: vi.fn() }));
vi.mock("./api", () => ({ api: { checkUpdateOnChannel: mocks.check, installPendingUpdate: mocks.install } }));
vi.mock("./dialogs.svelte", () => ({ dialogs: { confirm: mocks.confirm } }));
vi.mock("./overlays", () => ({ anyOverlayOpen: () => false }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let updater: typeof import("./updater.svelte");
let appState: (typeof import("./store.svelte"))["appState"];
let errors: (typeof import("./errors.svelte"))["errors"];
const update = { version: "0.4.0", currentVersion: "0.3.0", notes: null };
beforeEach(async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-09-26T12:00:00Z"));
  vi.stubEnv("DEV", false);
  vi.resetModules();
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.check.mockReset().mockResolvedValue(null);
  mocks.install.mockReset().mockResolvedValue(undefined);
  mocks.confirm.mockReset().mockResolvedValue(false);
  mocks.relaunch.mockReset().mockResolvedValue(undefined);
  ({ appState } = await import("./store.svelte"));
  vi.spyOn(appState, "waitForUpdateSettingsHydration").mockResolvedValue(undefined);
  vi.spyOn(appState, "getPersistedUpdateChannel").mockResolvedValue("stable");
  vi.spyOn(appState, "autoUpdateCheck", "get").mockReturnValue(true);
  ({ errors } = await import("./errors.svelte"));
  errors.reset();
  updater = await import("./updater.svelte");
});
afterEach(() => {
  errors.reset();
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("reports manual check failure with a popup", async () => {
  mocks.check.mockRejectedValue(new Error("fatal: network unavailable"));
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't check for updates", headline: "network unavailable" });
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
  expect(appState.busyOp).toBeNull();
});
it.each(["startup", "background", "activation"])("keeps %s check errors quiet but accessible", async (source) => {
  if (source === "background") {
    await updater.startupUpdateCheck();
    mocks.check.mockRejectedValue(new Error("fatal: offline"));
    await vi.advanceTimersByTimeAsync(30 * 60 * 1000);
  } else {
    mocks.check.mockRejectedValue(new Error("fatal: offline"));
    if (source === "startup") await updater.startupUpdateCheck();
    else await updater.updateCheckOnActivate();
  }
  expect(errors.current).toBeNull();
  expect(errors.pending).toBe(0);
  expect(errors.last).toMatchObject({ title: "Couldn't check for updates", headline: "offline" });
  expect(appState.statusIsError).toBe(true);
  expect(appState.logLines.join("\n")).toContain("offline");
});
it("notices only manual overlapping checks", async () => {
  let finish!: (value: null) => void;
  mocks.check.mockImplementation(() => new Promise<null>((resolve) => { finish = resolve; }));
  const first = updater.manualCheckForUpdates();
  await vi.waitFor(() => expect(mocks.check).toHaveBeenCalledOnce());
  await updater.updateCheckOnActivate();
  expect(errors.pending).toBe(0);
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't check for updates yet", headline: "Already checking for updates…" });
  expect(appState.statusIsError).toBe(false);
  finish(null);
  await first;
});
it("reports download/install failure and preserves signature guidance and raw output", async () => {
  mocks.check.mockResolvedValue(update);
  mocks.confirm.mockResolvedValue(true);
  mocks.install.mockRejectedValue(new Error("signature verification failed: artifact detail"));
  await updater.manualCheckForUpdates();
  expect(errors.current?.title).toBe("Couldn't download or install the update");
  expect(errors.current?.headline).toContain("Updater rejected the update");
  expect(errors.current?.output).toContain("signature verification failed: artifact detail");
  expect(errors.pending).toBe(1);
  expect(appState.busyOp).toBeNull();
});
it("reports relaunch rejection", async () => {
  mocks.check.mockResolvedValue(update);
  mocks.confirm.mockResolvedValue(true);
  mocks.relaunch.mockRejectedValue(new Error("fatal: relaunch denied"));
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({ title: "Couldn't restart to install the update", headline: "relaunch denied" });
  expect(errors.pending).toBe(1);
});
it("reports a relaunch watchdog once even if relaunch rejects later", async () => {
  let reject!: (e: unknown) => void;
  mocks.check.mockResolvedValue(update);
  mocks.confirm.mockResolvedValue(true);
  mocks.relaunch.mockImplementation(() => new Promise<void>((_, fail) => { reject = fail; }));
  const pending = updater.manualCheckForUpdates();
  await vi.waitFor(() => expect(mocks.relaunch).toHaveBeenCalledOnce());
  await vi.advanceTimersByTimeAsync(10000);
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't restart to install the update", headline: "Update install did not start. Verify the app is properly signed/notarized." });
  reject(new Error("late rejection"));
  await pending;
  expect(errors.pending).toBe(1);
  expect(appState.busyOp).toBeNull();
});
it("does not report user postponement as an error", async () => {
  mocks.check.mockResolvedValue(update);
  await updater.manualCheckForUpdates();
  expect(errors.pending).toBe(0);
  expect(appState.status).toBe("Update download postponed.");
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/updater.coverage.test.ts`

Expected: FAIL because checks and download/relaunch failures only set status, the watchdog only sets status, and manual overlap is not a notice.

- [ ] **Step 3: Implement updater reporting without changing scheduling**

The source discriminator already exists. `src-tauri/src/updater.rs`'s `install_pending_update` calls `download_and_install`; the renderer cannot reliably distinguish those two failing stages, so its title names both ("Couldn't download or install the update"). Relaunch has its own title ("Couldn't restart to install the update"). This is documentation evidence, not a Rust change.

`src/lib/updater.svelte.ts` — **Before**
```ts
import { api } from "./api";
```

**After**
```ts
import { api } from "./api";
import { errorText } from "./errorReport";
import { reportError, reportNotice } from "./errors.svelte";
```

`src/lib/updater.svelte.ts` — **Before**
```ts
    if (source === "manual") appState.status = "Already checking for updates…";
```

**After**
```ts
    if (source === "manual") reportNotice("Can't check for updates yet", "Already checking for updates…");
```

`src/lib/updater.svelte.ts` inside `checkForUpdates` only — **Before**
```ts
    reportUpdaterError(err);
```

**After**
```ts
    reportUpdaterError("Couldn't check for updates", err, { quiet: !manual });
```

`src/lib/updater.svelte.ts` — **Before**
```ts
  lastPromptedVersion = update.version;
  try {
```

**After**
```ts
  lastPromptedVersion = update.version;
  let failureTitle = "Couldn't download or install the update";
  let installReported = false;
  try {
```

`src/lib/updater.svelte.ts` — **Before**
```ts
    const restart = await dialogs.confirm({
```

**After**
```ts
    failureTitle = "Couldn't restart to install the update";
    const restart = await dialogs.confirm({
```

`src/lib/updater.svelte.ts` — **Before**
```ts
    const guard = setTimeout(() => {
      appState.status =
        "Update install did not start. Verify the app is properly signed/notarized.";
    }, 10000);
```

**After**
```ts
    const guard = setTimeout(() => {
      installReported = true;
      reportError(
        "Couldn't restart to install the update",
        "Update install did not start. Verify the app is properly signed/notarized.",
      );
    }, 10000);
```

`src/lib/updater.svelte.ts` inside `presentUpdate` only — **Before**
```ts
    reportUpdaterError(err);
```

**After**
```ts
    if (!installReported) reportUpdaterError(failureTitle, err);
```

`src/lib/updater.svelte.ts` — **Before**
```ts
function reportUpdaterError(err: unknown): void {
  const raw = err instanceof Error ? err.message : String(err);
  const sig = /signature|verify|verification|invalid/i.test(raw);
  appState.status = sig
    ? "Updater rejected the update (signature verification failed). The artifact may be unsigned or corrupted."
    : `Updater error: ${raw}`;
}
```

**After**
```ts
function reportUpdaterError(title: string, err: unknown, opts?: { quiet?: boolean }): void {
  const raw = errorText(err);
  const sig = /signature|verify|verification|invalid/i.test(raw);
  // The `error:` prefix makes the explanation the headline; the raw text follows it as a
  // continuation line and stays in the output.
  reportError(
    title,
    sig
      ? "error: Updater rejected the update (signature verification failed). " +
          "The artifact may be unsigned or corrupted.\n" +
          raw
      : err,
    opts,
  );
}
```

Keep `clearTimeout(guard)` in its existing `finally`, and keep `presenting`/busy-state cleanup, polling, activation throttling, hydration, deferred prompts, and channel seeding unchanged. `reportUpdaterError` receives `quiet` only for automatic checks; user-approved download/install/restart failures are always popups.

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/updater.coverage.test.ts src/lib/updater.test.ts`

Expected: PASS. The original updater lifecycle suite still covers startup, 30-minute polling, activation, overlay deferral, and superseded pending updates. Record real test counts when executing; this plan does not assert a pre-run count.

- [ ] **Step 5: Commit**

```bash
git add src/lib/updater.svelte.ts src/lib/updater.coverage.test.ts
git commit -F - <<'COMMIT'
fix: report manual updater failures and quiet automatic checks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 13: Report cancellation failures and quiet watcher-start failures

**Files:**
- Modify: `src/lib/gitActions.ts` (`cancelRemote`, `startWatchingRepo` only).
- Create/Test: `src/lib/gitActions.background.test.ts`.

**Interfaces:**
- Consumes: `api.cancelRemote(): Promise<void>`; `api.startWatch(repo: string, onChange: (kind: string) => void): Promise<void>`; PR 1's private `remoteCancelRequested` flag; `reportError(title: string, error: unknown, opts?: { quiet?: boolean }): void`.
- Produces: `gitActions.cancelRemote(): Promise<void>` consumes a cancellation rejection after reporting it; unchanged exported `startWatchingRepo(): Promise<void>` reports failures quietly.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/gitActions.background.test.ts`:

```ts
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { appState } from "./store.svelte";
import { errors } from "./errors.svelte";
import { dialogs } from "./dialogs.svelte";
import { gitActions, startWatchingRepo } from "./gitActions";
import type { RemoteOutcome } from "./types";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = "/tmp/watch";
  appState.setRepoLoading(false);
  appState.clearLog();
});
afterEach(() => {
  dialogs.resolvePrompt(null);
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
it("reports a cancellation rejection and consumes the promise", async () => {
  vi.spyOn(api, "cancelRemote").mockRejectedValue(new Error("fatal: cannot cancel"));
  await expect(gitActions.cancelRemote()).resolves.toBeUndefined();
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't cancel the remote operation", headline: "cannot cancel" });
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
});
it("preserves PR 1's cancellation flag even if the cancellation command rejects", async () => {
  let finish!: (result: RemoteOutcome) => void;
  vi.spyOn(api, "push").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  vi.spyOn(api, "cancelRemote").mockRejectedValue(new Error("fatal: cannot cancel"));
  vi.spyOn(api, "loadGraph").mockResolvedValue([]);
  vi.spyOn(api, "repoStatus").mockResolvedValue({ head: { sha: null, branch: null, detached: true }, staged: 0, unstaged: 0, untracked: 0, conflicted: 0, operation: null });
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue([]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
  vi.spyOn(api, "remotes").mockResolvedValue([]);
  const pushing = gitActions.push();
  await vi.waitFor(() => expect(api.push).toHaveBeenCalledOnce());
  await gitActions.cancelRemote();
  finish({ ok: false, authFailed: false, conflicted: false, message: "backend stopped" });
  await pushing;
  expect(errors.pending).toBe(1);
  expect(errors.current?.title).toBe("Couldn't cancel the remote operation");
  expect(appState.status).toBe("Push cancelled.");
});
it("reports watcher start quietly and leaves a pending prompt intact", async () => {
  vi.spyOn(api, "startWatch").mockRejectedValue(new Error("fatal: watch denied"));
  const prompt = dialogs.prompt({ title: "New branch", label: "Name" });
  await startWatchingRepo();
  expect(errors.current).toBeNull();
  expect(errors.pending).toBe(0);
  expect(errors.last).toMatchObject({ kind: "error", title: "Couldn't watch the repository", headline: "watch denied" });
  expect(appState.statusIsError).toBe(true);
  expect(appState.logLines.join("\n")).toContain("watch denied");
  expect(dialogs.state.kind).toBe("prompt");
  dialogs.resolvePrompt(null);
  await prompt;
});
it("keeps watcher startup inert without a repository", async () => {
  appState.repo = "";
  const start = vi.spyOn(api, "startWatch");
  await startWatchingRepo();
  expect(start).not.toHaveBeenCalled();
  expect(errors.pending).toBe(0);
  expect(errors.last).toBeNull();
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npx vitest run src/lib/gitActions.background.test.ts`

Expected: FAIL because PR 1's cancellation promise still rejects and watcher start only warns in the console.

- [ ] **Step 3: Implement the two reporting boundaries**

The reporting import is already present from PR 1 and Task 2; do not add a duplicate.

Current-tree evidence in `src/lib/gitActions.ts` (PR 1 replaces this before execution):

```ts
  cancelRemote: () => api.cancelRemote(),
```

**Explicit PR 1 overlap:** PR 1's required contract is `cancelRemote: () => { remoteCancelRequested = true; return api.cancelRemote(); }`. Replace that property with the following complete property, retaining its flag assignment:

```ts
  cancelRemote: () => {
    remoteCancelRequested = true;
    return api.cancelRemote().catch((e) => reportError("Couldn't cancel the remote operation", e));
  },
```

In `startWatchingRepo` only — **Before**

```ts
    console.warn("[gte] start watch failed", e);
```

**After**

```ts
    reportError("Couldn't watch the repository", e, { quiet: true });
```

Keep `stopWatchingRepo`, live refresh warnings, graph paging warnings, and refresh guards unchanged.

- [ ] **Step 4: Run tests to verify success**

Run: `npx vitest run src/lib/gitActions.background.test.ts src/lib/gitActions.partialFailure.test.ts`

Expected: PASS; the rejected cancel command has one popup, cancellation terminal handling remains PR 1's, and watcher failure only sets the red status/log/last error.

- [ ] **Step 5: Commit**

```bash
git add src/lib/gitActions.ts src/lib/gitActions.background.test.ts
git commit -F - <<'COMMIT'
fix: report cancellation and watcher startup failures

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

### Task 14: Gate sidebar notices and complete Part B verification

**Files:**
- Modify: `src/lib/components/Sidebar.svelte` (two guard messages only).
- Create/Test: `src/lib/components/Sidebar.notices.test.ts`.
- Read-only verification: all files listed in the File Structure map and the approved spec.

**Interfaces:**
- Consumes: `reportNotice(title: string, message: string): void`; `dialogs.prompt(opts: { title: string; label: string; value?: string; placeholder?: string; confirmLabel?: string }): Promise<string | null>`; `dialogs.resolvePrompt(v: string | null): void`.
- Produces: unchanged component-local `refActionsBlocked(): boolean` and `sameRepoAfterPrompt(): () => boolean`. This final task has its own red/green behavior change before running the complete release gates.

- [ ] **Step 1: Write the failing tests**

Create `src/lib/components/Sidebar.notices.test.ts`:

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { dialogs } from "../dialogs.svelte";
import { contextMenu } from "../contextMenu.svelte";
import { gitActions } from "../gitActions";
import { SAMPLE_GRAPH } from "../graph/sample";
import Sidebar from "./Sidebar.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/sidebar";
  appState.setGraphCommits([{ ...SAMPLE_GRAPH[0], refs: [{ name: "main", kind: "local", is_head: true }] }]);
  appState.setRefsDetailed([{ name: "main", kind: "local", target_sha: SAMPLE_GRAPH[0].sha, upstream: null, ahead: 0, behind: 0 }]);
  appState.setRemotes([]);
  appState.setWorktrees([]);
  appState.setRepoLoading(false);
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  dialogs.resolvePrompt(null);
  await dispose?.();
  dispose = undefined;
  contextMenu.close();
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("notices loading refusal before opening a ref context menu", async () => {
  appState.setRepoLoading(true);
  const panel = mount(Sidebar, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector('button.ref[title="main"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  expect(contextMenu.open).toBe(false);
  expect(errors.current).toMatchObject({ kind: "notice", title: "Repository still opening", headline: "Still opening the repository — try that again in a moment." });
  expect(appState.statusIsError).toBe(false);
});
it("notices changed repository after a rename prompt and makes no write", async () => {
  const rename = vi.spyOn(gitActions, "renameBranch");
  const panel = mount(Sidebar, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector('button.ref[title="main"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  const pending = contextMenu.items.find((item) => item.label === "Rename…")!.action!();
  expect(dialogs.state.kind).toBe("prompt");
  appState.repo = "/tmp/other-sidebar";
  dialogs.resolvePrompt("renamed");
  await pending;
  expect(rename).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Repository changed", headline: "Repository changed while that dialog was open — nothing was done." });
  expect(errors.pending).toBe(1);
  expect(appState.status).toBe("Repository changed while that dialog was open — nothing was done.");
});
```

- [ ] **Step 2: Run the final task's test to verify failure**

Run: `npx vitest run src/lib/components/Sidebar.notices.test.ts`

Expected: FAIL because Sidebar's two guards remain status-only until this task. Earlier tasks do not modify Sidebar.

- [ ] **Step 3: Implement Sidebar's two notice sites**

`src/lib/components/Sidebar.svelte` — **Before**
```ts
  import { appState } from "../store.svelte";
```

**After**
```ts
  import { appState } from "../store.svelte";
  import { reportNotice } from "../errors.svelte";
```

`src/lib/components/Sidebar.svelte` — **Before**
```ts
    appState.status = "Still opening the repository — try that again in a moment.";
```

**After**
```ts
    reportNotice("Repository still opening", "Still opening the repository — try that again in a moment.");
```

`src/lib/components/Sidebar.svelte` — **Before**
```ts
      appState.status = "Repository changed while that dialog was open — nothing was done.";
```

**After**
```ts
      reportNotice("Repository changed", "Repository changed while that dialog was open — nothing was done.");
```

- [ ] **Step 4: Run the gate**

First run: `npx vitest run src/lib/components/Sidebar.notices.test.ts src/lib/gitActions.notices.test.ts`

Expected: PASS. Then run every required gate separately, from the repository root:

```bash
npm run check
npm test
cargo test
cargo clippy -p git-core -p git-it -- -D warnings
```

Expected: `npm run check` exits 0 with **0 errors**; Vitest exits 0 with no failing tests or unhandled errors; every Cargo workspace test passes; clippy exits 0 with warnings denied. Record actual counts, warnings, and environmental blockers at execution time. A blocked or unrun gate is not green. The planning pass itself does not run these commands or create test artifacts.

Review the actual feature diff against the PR 2 starting point (updated `next`):

```bash
git diff --check
git diff next...HEAD -- src/lib/gitActions.ts
git diff -- src/lib/components/Sidebar.svelte src/lib/components/Sidebar.notices.test.ts
git status --short
```

Expected: no whitespace errors; no hunk edits `gitActions.pull`; only the planned production/test paths changed. Check the named functions rather than doing a file-wide replacement of repeated messages. Confirm no `dialogs.alert`/`resolveAlert` was reintroduced and no wrapper caller gained a second report.

In a running debug app against scratch repositories, with the installed Git It quit, verify the Part B UI in **Classic and NERV**. Start the debug app with:

```bash
npm run tauri dev
```

Exercise Open/Open Recent with a folder that is not a repository; check its notice body and normal status color. Right-click a branch with a mixed-case name while a repository is still opening and confirm the title keeps its case. Open a working-copy file context menu, remove that scratch file externally before selecting Open, and verify a readable error with raw details in Output. Use Copy SHA and confirm a successful copy remains unobtrusive. Check Stash, Reflog, Conflict, Backups and diff Retry states against the unit-test fixtures and verify long/selectable text, keyboard-reachable Retry, and panel spacing at a narrow window width. Expand the Stash panel and touch a file in the working tree: the list must not flash "Loading stashes…" on the watcher refresh. For native IPC failures that cannot be induced safely in the debug app, record the mocked-test coverage and the unrun native scenario explicitly. Debug builds skip automatic updates; the updater failure matrix is tested through the mocked non-dev module and is not evidence of a real signed-artifact install.

Do not add Pull strategy/autostash scenarios to this PR's implementation work; those belong to PR 3. PR 1's error-dialog/status-bar behavior remains a regression dependency, not another implementation task.

- [ ] **Step 5: Commit**

After all gates and scope review pass:

```bash
git add src/lib/components/Sidebar.svelte src/lib/components/Sidebar.notices.test.ts
git commit -F - <<'COMMIT'
fix: complete sidebar refusal coverage and verification

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
COMMIT
```

## Self-review

### B1: Swallowed action failures

| Required bullet/site | Coverage |
|---|---|
| `repositoryFlows.openRepositoryFlow`: picker rejection | Task 1, picker test and whole-function catch |
| `repositoryFlows.openRepositoryFlow`: `isGitRepo` rejection | Task 1, validation test and same catch |
| RepoList open-from-recents rejection | Task 1, component test and `openRecentRepo` replacement |
| RepoTabs open-from-recents rejection | Task 1, component test and `openRecent` replacement |
| WorkingCopy Open | Task 5, opener catch and UI-context test |
| WorkingCopy Open With | Task 5, app-specific opener catch and argument test; app discovery omission remains unchanged |
| WorkingCopy Show in Finder, including deleted-file parent fallback | Task 5, both opener/reveal branches tested |
| Open Worktree Folder | Task 5 |
| `gitActions.cancelRemote` rejection while retaining PR 1 flag | Task 13, direct rejection plus in-flight remote cancellation tests |
| GraphHistory Copy SHA | Task 5, rejected and absent clipboard |
| PrCommitsTab Copy SHA | Task 5, rejected and absent clipboard, no false success chip |
| `setCommitMessage.countMergesInRange` precheck rejection | Task 3 |
| `setCommitMessage.rebaseTodoPreview` precheck rejection | Task 3; wrapper ownership checked on both execution paths |
| CommitComposer HEAD-message load | Task 4; checkbox unticks, draft returns, stale error ignored |
| CommitMessageEdit message load | Task 4 |
| ApplyPanel rewrite failure | Task 6; streamed/lifecycle logs retained, raw error logged once |
| Backups create bundle | Task 10 |
| Backups delete bundle | Task 10 |
| Backups fetch from bundle | Task 10 |
| Backups delete selected safety refs | Task 10 |
| Backups delete all safety refs | Task 10 |
| Manual update check, Settings and macOS menu | Task 12; both existing callers verified, same exported function |
| Update download/install command | Task 12; combined backend operation and signature detail tested |
| Update relaunch/watchdog | Task 12; popup and no duplicate after timeout |

### B2: Refusals

| Required bullet/site | Coverage |
|---|---|
| Open-repository guards in every wrapper | Task 2: `run`, `runOp`, `runResolve`, `runWorktree`, `runDestructive`, `runDestructiveRebase`, `runRemote` |
| Open-repository guards in direct actions | Task 2: `discardHunk`, `discardLines`, `amendCommit`, `discard`, `clean`; Task 3 also guards reword probes before IPC |
| Fetch/push loading guards | Task 2; entire Pull export excluded |
| Concurrent remote operation | Task 2 `runRemote` |
| `sameRepoAfterPrompt` in gitActions | Task 2 |
| Sidebar repository loading and changed during prompt | Task 14, both observable through ref menus |
| Reword root / merge / range merge / absent history | Task 3, all four exact bodies |
| Undo changed branch | Task 2, snapshot still cleared |
| Create-PR detached / no base | Task 2, both exact bodies |
| Non-git repository at all three open sites | Task 1 |
| Repository creation already active | Task 1 |
| ApplyPanel unavailable prerequisites / no dates | Task 6, defensive handler tests |
| EditTabs selection / exact invalid / compress count / compress invalid / ordering / unreadable dates | Task 6, all six sites; repeated invalid-date text replaced in both functions |
| RebaseTodo all-drop | Task 6 |
| Already checking for updates, manual only | Task 12 |
| Previously silent `runResolve`, no desktop / no repository | Task 2: desktop-only status versus repository notice |
| Previously silent squash-merge busy | Task 2 |
| Excluded desktop-preview strings and squash combined guard | Task 1 and Task 2 explicitly preserve and test them; no popup conversion |
| Required site → title → message table | Refusal copy table above; implementations use those strings |

### B3: Retryable inline loads

| Required bullet/site | Coverage |
|---|---|
| One shared component, summary first line, Retry, specified CSS tokens and selectable text | Task 7 `LoadError`; no duplicate inline error markup |
| Stash list | Task 7, pending/error/retry/success-empty and stale response |
| Reflog list | Task 7, pending/error/retry/success-empty and stale response |
| Conflict details | Task 8, including no misleading resolved badge/actions |
| Working-copy file diff, matching compact CommitDetail pattern | Task 9; unchanged DiffView/CommitDetail internals |
| Bundle list including automatic `$effect` load | Task 10 |
| Safety-ref list | Task 10; active list refreshes on repository switch |
| GitHub availability actual error and Retry | Task 11: state → GithubView → GithubSetupCard → shared LoadError |
| Genuine NotInstalled / NotAuthed / NoRemote unchanged | Task 11 tests all three returned states |
| Empty copy only after successful zero-item load | Tasks 7–10: `loaded` flips only on success; GitHub uses availability rather than item count |
| A refresh keeps the current list (no "Loading" flash on watcher/undo/action refreshes) | Tasks 7 and 10 refresh tests; no loader resets `loaded` |

### B4: Quiet reports

| Required bullet/site | Coverage |
|---|---|
| Startup update check | Task 12, source-specific quiet test |
| 30-minute polling update check | Task 12, fake-timer quiet test |
| Activation update check | Task 12, quiet test plus existing lifecycle suite |
| Filesystem watcher start failure | Task 13; red status/log/last error, no popup, pending prompt intact |

### B5: Unchanged

| Required bullet/site | Preservation check |
|---|---|
| Settings-persistence failures (console) | No persistence code touched; store mocks isolate tests |
| GitHub list panels (already inline) | Task 11 only changes availability state and setup wiring |
| GitHub write actions (already inline, write modals stay open) | No GitHub action/write-modal changes |
| Shiki highlight fallback | No DiffView/highlighter changes |
| `tauriDrag` | No window-drag helper changes |
| localStorage reads | No settings/store implementation changes |
| Graph paging failures | Task 13 only changes watcher start; paging/refresh console behavior preserved |

### Plan integrity checks

- All fourteen tasks include exact touched files, consumed/produced interfaces, complete test-file code, an expected red command, exact implementations, a green command, and a conventional commit heredoc with the required final co-author line.
- Every production replacement is grounded in the read current tree. PR 1 overlaps are explicit: reporting imports, wrapper catch preservation, cancellation flag, and alert-based tests owned by PR 1.
- Shared names agree: `LoadError` uses `what`, `error`, `onRetry`; `GithubSetupCard` preserves `onretry` and adds `availabilityError`; updater keeps its existing `source` discriminator; all reporting uses the supplied PR 1 names; every `what` reads after "Couldn't load"; `cantTitle` is the one label-derived title.
- Adversarial review (2026-09-26): every **Before** quotation was re-checked against the tree at `340ee99` (and PR 1's plan text for the two PR 1-result quotations); jsdom 29.1.1 was confirmed to lack `window.matchMedia` and `Element.prototype.animate`, which is why the WorkingCopyView tests stub `matchMedia` and the history tests mock `slide` to a zero-duration transition (Svelte skips `element.animate` only then). Test files are type-checked by `npm run check` (`.svelte-kit/tsconfig.json` includes `src/**/*.ts`), so the history tests mount each panel through its own closure rather than a union of component types.
- No implementation, staging, commits, build, app launch, or runtime tests were performed to write this plan. Future test expectations are not verification results.
- Scope gaps: none identified for B1–B5. Native runtime validation remains a future gate, with blocked/unrun scenarios to be reported honestly during execution.
