// Action layer over the Tauri op commands: each action guards for the desktop
// app, runs the op, refreshes the graph, and reports status — so components just
// call gitActions.checkout(...) etc. without repeating that plumbing.
import { appState } from "./store.svelte";
import { api } from "./api";
import { SAMPLE_GRAPH } from "./graph/sample";
import type { GraphCommit, OpOutcome, RemoteOutcome, RewriteResult, RebaseOutcome, RebaseStep, UndoSnapshot, WorkingFile } from "./types";
import { dialogs } from "./dialogs.svelte";
import { graphView } from "./graphView.svelte";
import { githubState } from "./githubState.svelte";
import { prefillFromSubjects } from "./github/prCreate";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const PAGE = 150;

// Coordinate preserving graph refreshes with incremental page loads. A failed
// operation may still have changed refs (for example, local delete succeeded but
// remote delete failed), so its refresh must supersede any page fetched from the
// pre-operation graph rather than letting stale decorations append afterward.
let graphRefreshGeneration = 0;
let graphPageLoadPromise: Promise<void> | null = null;
let preservingGraphRefreshPromise: Promise<void> | null = null;
let graphReloadPromise: Promise<void> | null = null;
let preservingGraphRefreshQueued = false;

export async function refreshStatus(): Promise<void> {
  if (!isTauri() || !appState.repo) {
    appState.setRepoStatus(null);
    return;
  }
  try {
    appState.setRepoStatus(await api.repoStatus(appState.repo));
  } catch (e) {
    console.warn("[gte] repo status refresh failed", e);
    appState.setRepoStatus(null);
  }
}

export async function refreshWorkingChanges(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  try {
    appState.setWorkingChanges(await api.workingChanges(appState.repo));
  } catch (e) {
    console.warn("[gte] working changes refresh failed", e);
  }
}

// Refresh the detailed ref list (ahead/behind/upstream), remotes, and worktrees.
// Tauri-only; silently no-ops in browser preview.
export async function refreshRefs(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  const target = appState.repo;
  const [refs, remotes, worktrees] = await Promise.allSettled([
    api.listRefs(target),
    api.remotes(target),
    api.listWorktrees(target),
  ]);
  if (appState.repo !== target) return;
  if (refs.status === "fulfilled") appState.setRefsDetailed(refs.value, target);
  else {
    appState.invalidateRefsDetailed(target);
    console.warn("[gte] refs refresh failed", refs.reason);
  }
  if (remotes.status === "fulfilled") appState.setRemotes(remotes.value);
  else console.warn("[gte] remotes refresh failed", remotes.reason);
  if (worktrees.status === "fulfilled") appState.setWorktrees(worktrees.value);
  else console.warn("[gte] worktrees refresh failed", worktrees.reason);
}

// ── Live working-copy watching (filesystem watcher + focus refresh) ───────────
// Keeps the Local Changes list current as files change on disk, instead of only
// after explicit git ops. The Rust watcher pushes a debounced "changed" event;
// onFsChange then refreshes the working copy + status (a real change re-fetches
// the selected file's diff too, which is correct). A tiny extra debounce batches
// back-to-back events into a single refresh.
// Coalesced + change-guarded refresh of the working copy + status. Shared by the
// filesystem watcher and the focus/visibility refresh. The equality guard is
// essential: the watcher fires on ANY worktree write — including churn in ignored
// dirs (node_modules/, target/, editor temp files) that `git status` filters out
// — so we only REPLACE workingChanges (which bumps workingChangesRev and re-fetches
// the open diff) when the file list ACTUALLY changed; otherwise the diff pane would
// flash on every tick. Explicit ops keep using refreshWorkingChanges() directly,
// which always bumps the rev — hunk/line staging deliberately relies on that.
function sameWorkingChanges(a: WorkingFile[], b: WorkingFile[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const x = a[i];
    const y = b[i];
    if (
      x.path !== y.path ||
      x.status !== y.status ||
      x.staged !== y.staged ||
      x.unstaged !== y.unstaged ||
      x.untracked !== y.untracked ||
      x.conflicted !== y.conflicted
    )
      return false;
  }
  return true;
}

let localRefreshDebounce: ReturnType<typeof setTimeout> | null = null;
function scheduleLocalRefresh(): void {
  if (localRefreshDebounce) clearTimeout(localRefreshDebounce);
  localRefreshDebounce = setTimeout(async () => {
    localRefreshDebounce = null;
    if (!isTauri() || !appState.repo) return;
    try {
      const next = await api.workingChanges(appState.repo);
      if (!sameWorkingChanges(appState.workingChanges, next)) appState.setWorkingChanges(next);
    } catch (e) {
      console.warn("[gte] local refresh failed", e);
    }
    void refreshStatus();
  }, 80);
}

// A LIVE refresh of the graph for the filesystem watcher / window focus. Unlike
// reloadGraph (the repo-switch reset, which reloads the first page and CLEARS the
// selection/queued edits), this reloads up to the currently-loaded depth and
// applies it NON-destructively — preserving the open commit, multi-selection,
// queued time-edits and scroll. It also skips while a page-load is in flight so a
// background event can't interleave into a non-contiguous graph.
function refreshGraphPreservingState(waitForPageLoad: boolean): Promise<void> {
  if (!isTauri() || !appState.repo) return Promise.resolve();
  if (graphReloadPromise) {
    // A full reload owns the canonical graph replacement. A required refresh
    // runs after it; ordinary watcher events coalesce into one queued snapshot.
    if (waitForPageLoad) {
      return graphReloadPromise.then(() => refreshGraphPreservingState(true));
    }
    preservingGraphRefreshQueued = true;
    return graphReloadPromise;
  }
  if (preservingGraphRefreshPromise) {
    // A failed op needs a snapshot requested after that op completed. If a
    // watcher/focus refresh was already in flight, let it settle and then take
    // another snapshot rather than accepting its potentially pre-op result.
    if (waitForPageLoad) {
      return preservingGraphRefreshPromise.then(() => refreshGraphPreservingState(true));
    }
    // The active request may have captured its snapshot before this newer
    // watcher/focus event. Remember one dirty bit so the event is not lost.
    preservingGraphRefreshQueued = true;
    return preservingGraphRefreshPromise;
  }
  if (appState.graphLoadingMore && !waitForPageLoad) {
    preservingGraphRefreshQueued = true;
    return graphPageLoadPromise ?? Promise.resolve();
  }

  const task = (async () => {
    const target = appState.repo;
    if (!target) return;

    if (appState.graphLoadingMore) {
      // Invalidate the page before waiting: its response was requested against
      // the old ref state and must not append even if the commit count is equal.
      graphRefreshGeneration += 1;
      const pendingPage = graphPageLoadPromise;
      if (pendingPage) await pendingPage;
      else {
        while (appState.repo === target && appState.graphLoadingMore) {
          await new Promise((resolve) => setTimeout(resolve, 20));
        }
      }
    }
    if (appState.repo !== target) return;

    const generation = ++graphRefreshGeneration;
    const count = Math.max(PAGE, appState.graphCommits.length);
    let gc: GraphCommit[] | null = null;
    try {
      gc = await api.loadGraph(target, count, 0);
    } catch (e) {
      console.warn("[gte] preserving graph refresh failed", e);
    }
    if (appState.repo !== target || generation !== graphRefreshGeneration) return;
    if (gc) {
      appState.applyGraphRefresh(gc);
      appState.setGraphHasMore(gc.length >= count);
    }
    await refreshStatus();
    if (appState.repo !== target) return;
    await refreshWorkingChanges();
    if (appState.repo !== target) return;
    await refreshRefs();
  })();

  const tracked = task.finally(() => {
    if (preservingGraphRefreshPromise === tracked) {
      preservingGraphRefreshPromise = null;
      runQueuedPreservingGraphRefresh();
    }
  });
  preservingGraphRefreshPromise = tracked;
  return tracked;
}

function runQueuedPreservingGraphRefresh(): void {
  if (
    !preservingGraphRefreshQueued ||
    preservingGraphRefreshPromise ||
    graphReloadPromise ||
    graphPageLoadPromise
  ) {
    return;
  }
  preservingGraphRefreshQueued = false;
  void refreshGraphPreservingState(false).catch((e) => {
    console.warn("[gte] queued graph refresh failed", e);
  });
}

async function liveRefreshGraph(): Promise<void> {
  await refreshGraphPreservingState(false);
}

// Coalesced live refresh. Triggered by a "git" filesystem event (a commit/branch/
// checkout/fetch/merge/reset made by the app OR externally) and by the focus/
// visibility refresh.
let graphRefreshDebounce: ReturnType<typeof setTimeout> | null = null;
function scheduleGraphRefresh(): void {
  if (graphRefreshDebounce) clearTimeout(graphRefreshDebounce);
  graphRefreshDebounce = setTimeout(() => {
    graphRefreshDebounce = null;
    void liveRefreshGraph();
  }, 120);
}

// Route a classified watcher event. A "git" ref/HEAD change reloads the graph
// (which also covers refs/status/working copy); a "local" worktree/index change
// does the lighter working-copy refresh.
function onWatchEvent(kind: string): void {
  if (kind === "git" || kind === "both") scheduleGraphRefresh();
  else scheduleLocalRefresh();
}

// Start (or restart) the filesystem watcher for the current repo. Safe to call
// repeatedly — the backend replaces any existing watcher.
export async function startWatchingRepo(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  try {
    await api.startWatch(appState.repo, onWatchEvent);
  } catch (e) {
    console.warn("[gte] start watch failed", e);
  }
}

export async function stopWatchingRepo(): Promise<void> {
  if (!isTauri()) return;
  try {
    await api.stopWatch();
  } catch (e) {
    console.warn("[gte] stop watch failed", e);
  }
}

// Focus/visibility refresh: on returning to the window the user may have
// committed or edited in another app while we were backgrounded. Reload the
// graph (which also refreshes refs/status/working copy) so the timeline reflects
// external changes on return — not only the working-copy list. Debounced, so a
// re-activation firing BOTH focus and visibilitychange still refreshes once.
export function refreshActiveRepo(): void {
  scheduleGraphRefresh();
}

export async function reloadGraph(): Promise<void> {
  if (!isTauri()) {
    appState.setGraphCommits(SAMPLE_GRAPH);
    appState.setGraphHasMore(false);
    return;
  }
  // Capture the repo this load is for. If a newer switch supersedes it mid-load,
  // bail before committing results so we never overwrite the current repo's data
  // with a stale repo's (the flicker fix keeps the old data visible until here).
  const target = appState.repo;
  if (!target) return;
  const generation = ++graphRefreshGeneration;
  const task = (async () => {
    try {
      let gc: GraphCommit[];
      try {
        gc = await api.loadGraph(target, PAGE, 0);
      } catch (e) {
        // Load failed (repo moved / deleted / corrupt). Because the flicker fix keeps
        // the PREVIOUS repo's data on screen until this point, we must clear it on
        // failure for the still-current repo — otherwise one repo's history would show
        // under another's name. Bail silently if a newer switch already superseded us.
        if (appState.repo === target && generation === graphRefreshGeneration) {
          // Any page requested while this reload was in flight was based on the
          // failed snapshot and must not append after we clear the graph.
          graphRefreshGeneration += 1;
          appState.setGraphCommits([]);
          appState.setGraphHasMore(false);
          appState.setRepoStatus(null);
          appState.setRefsDetailed([]);
          appState.setWorktrees([]);
          appState.setWorkingChanges([]);
          appState.status = `Could not open ${target}: ${e}`;
        }
        return;
      }
      if (appState.repo !== target || generation !== graphRefreshGeneration) return;
      // Invalidate pages that started before or during this reload. The first
      // increment above rejects older requests; this one rejects requests made
      // while loadGraph itself was still in flight.
      graphRefreshGeneration += 1;
      // Once the new repository's graph replaces the old one, old worktree paths
      // must no longer remain visible or actionable while their refresh settles.
      // Same-repository reloads keep the cached list on transient failures.
      if (appState.repoLoading) appState.setWorktrees([]);
      appState.setGraphCommits(gc);
      appState.setGraphHasMore(gc.length === PAGE);
      await refreshStatus();
      if (appState.repo !== target) return;
      await refreshWorkingChanges();
      if (appState.repo !== target) return;
      await refreshRefs();
    } finally {
      // Clear the switch-in-progress flag (set by `set repo`) so remote actions
      // re-enable — but only if we're still the current repo, so a superseding
      // switch's own flag isn't cleared out from under it.
      if (appState.repo === target) appState.setRepoLoading(false);
    }
  })();
  const tracked = task.finally(() => {
    if (graphReloadPromise === tracked) {
      graphReloadPromise = null;
      runQueuedPreservingGraphRefresh();
    }
  });
  graphReloadPromise = tracked;
  await tracked;
}

export async function loadMoreGraph(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  if (!appState.graphHasMore || appState.graphLoadingMore || preservingGraphRefreshPromise) return;
  appState.setGraphLoadingMore(true);
  const target = appState.repo;
  const offset = appState.graphCommits.length;
  const generation = graphRefreshGeneration;
  const task = (async () => {
    try {
      const gc = await api.loadGraph(target, PAGE, offset);
      // If a preserving refresh or repo switch superseded this request, discard
      // its pre-refresh decorations even when the graph length stayed the same.
      if (
        appState.repo !== target ||
        graphRefreshGeneration !== generation ||
        appState.graphCommits.length !== offset
      )
        return;
      appState.appendGraphCommits(gc);
      appState.setGraphHasMore(gc.length === PAGE);
    } catch (e) {
      console.warn("[gte] load more failed", e);
    } finally {
      appState.setGraphLoadingMore(false);
    }
  })();
  const tracked = task.finally(() => {
    if (graphPageLoadPromise === tracked) {
      graphPageLoadPromise = null;
      runQueuedPreservingGraphRefresh();
    }
  });
  graphPageLoadPromise = tracked;
  await tracked;
}

// Jump to a commit by SHA, paging in more history first if it isn't loaded yet.
// A sidebar ref (tag/branch) can point at a commit far below the currently-loaded
// graph window; without this the scroll target wouldn't exist and the click would
// silently do nothing. We load page by page until the SHA appears, history is
// exhausted, or we stop making progress, then scroll the graph to it.
export async function jumpToRefWithLoad(targetSha: string): Promise<void> {
  // Browser preview (no Tauri): best-effort scroll within the sample graph.
  if (!isTauri() || !appState.repo) {
    graphView.scrollToCommit(targetSha);
    return;
  }
  const has = () => appState.graphCommits.some((c) => c.sha === targetSha);
  if (has()) {
    graphView.scrollToCommit(targetSha);
    return;
  }
  const repo = appState.repo; // bail if the user switches repos mid-load
  appState.status = "Loading history to that commit…";
  // stalls guards against an in-flight concurrent load (loadMoreGraph no-ops while
  // one is running) or a failing page (which leaves graphHasMore true) — either way
  // the commit count won't grow, so we back off briefly and cap the retries.
  let stalls = 0;
  while (appState.repo === repo && appState.graphHasMore && !has()) {
    const before = appState.graphCommits.length;
    await loadMoreGraph();
    if (appState.repo !== repo) return;
    if (appState.graphCommits.length === before) {
      if (++stalls > 40) break;
      await new Promise((r) => setTimeout(r, 50));
    } else {
      stalls = 0;
    }
  }
  if (appState.repo !== repo) return;
  if (has()) {
    graphView.scrollToCommit(targetSha);
    appState.status = "";
  } else {
    appState.status = "Couldn't locate that commit in this repository's history.";
  }
}

// Run an op with uniform guard / status / refresh / error handling.
async function run(label: string, fn: () => Promise<unknown>): Promise<boolean> {
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return false;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return false;
  }
  appState.setNavBusy(true);
  appState.setBusyOp(label);
  try {
    appState.status = `${label}…`;
    await fn();
    try { await reloadGraph(); } catch (e) { console.warn("[gte] graph refresh after op failed", e); }
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    // A partial success (e.g. the local branch was deleted but its remote delete failed)
    // should update every graph decoration as well as the sidebar. Use the
    // preserving refresh: reloadGraph()/setGraphCommits() would clear queued
    // commit-time edits, selection, and currentSha after an unrelated failure.
    try {
      await refreshGraphPreservingState(true);
    } catch (err) {
      console.warn("[gte] preserving graph refresh after failed op", err);
    }
    appState.status = `${label} failed: ${firstLine(e)}`;
    // The status bar alone is too easy to miss for a discrete action the user just took
    // (user-reported "no feedback for if something went wrong") — surface it as a dialog.
    void dialogs.alert({ title: `${label} failed`, message: errorText(e) });
    return false;
  } finally {
    appState.setNavBusy(false);
    appState.setBusyOp(null);
  }
}

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

// Run an OpOutcome-returning op. Always refreshes graph+status (even on error) so
// the conflict panel can offer recovery (contract a). Summarizes status (contract d).
async function runOp(label: string, fn: () => Promise<OpOutcome>): Promise<OpOutcome | null> {
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return null;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return null;
  }
  appState.status = `${label}…`;
  try {
    const outcome = await fn();
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] graph refresh after op failed", e);
    }
    appState.status = outcome.conflicted
      ? `${label}: ${outcome.files.length} conflict(s) to resolve.`
      : `${label} — done.`;
    return outcome;
  } catch (e) {
    // Refresh so a half-finished op surfaces in the conflict panel for recovery.
    try {
      await reloadGraph();
    } catch (re) {
      console.warn("[gte] graph refresh after failed op failed", re);
    }
    appState.status = `${label} failed: ${firstLine(e)}`;
    return null;
  }
}

// Resolving a single file changes neither the commit graph nor selection/edits, so
// refresh only status (which drives the conflict list) — never reloadGraph here.
async function runResolve(label: string, fn: () => Promise<unknown>): Promise<boolean> {
  if (!isTauri() || !appState.repo) return false;
  try {
    await fn();
    await refreshStatus();
    return true;
  } catch (e) {
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}

// Run a working-copy op (non-destructive): guard → run → refresh working changes + graph.
async function runWorktree(label: string, fn: () => Promise<unknown>): Promise<boolean> {
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return false;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return false;
  }
  try {
    appState.status = `${label}…`;
    await fn();
    // reloadGraph already refreshes the working changes (+ status, graph, refs).
    // We deliberately do NOT also refresh working changes separately here: a second
    // setWorkingChanges() landing mid-flight would interrupt the stage/unstage move
    // animation (the row would flicker and pop instead of flying). One refresh only.
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] graph refresh after working-copy op failed", e);
    }
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}

// Confirm (with consequence + backup choice) → run → store undo → refresh. Returns ok.
async function runDestructive(
  label: string,
  consequence: string,
  fn: (backup: boolean) => Promise<RewriteResult>,
): Promise<boolean> {
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return false;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return false;
  }
  const { confirmed, backup } = await dialogs.confirmDestructive({
    title: label,
    consequence,
    confirmLabel: label,
    backupDefault: appState.autoBackupDestructive,
  });
  if (!confirmed) return false;
  try {
    appState.status = `${label}…`;
    const res = await fn(backup);
    appState.setLastUndo(res.undo);
    if (res.bundle) appState.appendLog(`[backup] ${res.bundle}`);
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] refresh failed", e);
    }
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    try {
      await reloadGraph();
    } catch {}
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}

// Rebase variant: same confirm+undo, but the result is a RebaseOutcome that may conflict.
async function runDestructiveRebase(
  label: string,
  consequence: string,
  fn: (backup: boolean) => Promise<RebaseOutcome>,
): Promise<boolean> {
  if (!isTauri()) {
    appState.status = "That action needs the desktop app (not the browser preview).";
    return false;
  }
  if (!appState.repo) {
    appState.status = "Open a repository first.";
    return false;
  }
  const { confirmed, backup } = await dialogs.confirmDestructive({
    title: label,
    consequence,
    confirmLabel: label,
    backupDefault: appState.autoBackupDestructive,
  });
  if (!confirmed) return false;
  try {
    appState.status = `${label}…`;
    const res = await fn(backup);
    appState.setLastUndo(res.undo);
    if (res.bundle) appState.appendLog(`[backup] ${res.bundle}`);
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] refresh failed", e);
    }
    appState.status = res.outcome.conflicted
      ? `${label}: ${res.outcome.files.length} conflict(s) to resolve.`
      : appState.repoStatus?.operation
        ? "Rebase paused — amend the commit if needed, then continue."
        : `${label} — done.`;
    return true;
  } catch (e) {
    try {
      await reloadGraph();
    } catch {}
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}

// ── Remote operation helper (Phase 6) ────────────────────────────────────────
// Runs a streamed pull/push op with:
//   1. Progress state management (startRemoteProgress / pushRemoteLog / endRemoteProgress)
//   2. First attempt WITHOUT credentials (relies on system credential helper / SSH agent)
//   3. On authFailed: prompt for credentials and retry
//   4. Graph reload + status update (conflicted → ConflictView; ok → done)
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
  appState.startRemoteProgress(label);
  try {
    // First attempt: no credentials (system helper / SSH agent / keychain).
    let outcome = await fn((l) => appState.pushRemoteLog(l));

    if (outcome.authFailed) {
      // Auth failed — prompt for credentials and retry once.
      const creds = await dialogs.confirmCredentials({ title: `${label}: sign in` });
      if (creds) {
        outcome = await fn((l) => appState.pushRemoteLog(l), creds);
      }
      // If user cancelled the credentials dialog, fall through with the original outcome.
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
    } else {
      appState.status = `${label} failed: ${firstLine(outcome.message)}`;
    }

    return outcome.ok;
  } catch (e) {
    try {
      await reloadGraph();
    } catch {}
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  } finally {
    appState.endRemoteProgress();
  }
}

export const gitActions = {
  checkout: (target: string, label?: string) =>
    run(label ?? `Checkout ${target}`, () => api.checkout(appState.repo, target)),
  createBranch: (name: string, startPoint: string) =>
    run(`Create branch ${name}`, () => api.createBranch(appState.repo, name, startPoint)),
  renameBranch: (oldName: string, newName: string) =>
    run(`Rename ${oldName} → ${newName}`, () => api.renameBranch(appState.repo, oldName, newName)),
  deleteBranch: async (
    name: string,
    force: boolean,
    deleteRemote = false,
    remote?: string,
    remoteBranch?: string,
    worktreePath?: string,
  ) => {
    const repo = appState.repo;
    const ok = await run(`Delete branch ${name}`, () =>
      api.deleteBranch(repo, name, force, deleteRemote, remote, remoteBranch, worktreePath),
    );
    if (worktreePath && repo && isTauri()) {
      if (ok) {
        // Backend success proves the requested linked worktree was removed.
        appState.forgetRepo(worktreePath);
      } else {
        try {
          // The backend removes the linked worktree before a requested remote
          // delete. Reconcile after an error so that partial success clears a
          // stale tab, without forgetting a worktree whose removal was refused.
          const worktrees = await api.listWorktrees(repo);
          if (!worktrees.some((worktree) => worktree.path === worktreePath)) {
            appState.forgetRepo(worktreePath);
          }
        } catch (error) {
          console.warn("[gte] could not verify worktree removal after branch deletion", error);
        }
      }
    }
    return ok;
  },
  removeWorktree: async (path: string) => {
    let removedPath: string | null = null;
    const ok = await run("Remove worktree", async () => {
      removedPath = await api.removeWorktree(appState.repo, path);
    });
    if (ok && removedPath) appState.forgetRepo(removedPath);
    return ok;
  },
  deleteRemoteBranch: (remote: string, branch: string) =>
    run(`Delete remote branch ${remote}/${branch}`, () =>
      api.deleteRemoteBranch(appState.repo, remote, branch),
    ),
  fastForwardBranch: (branch: string, remote: string, remoteBranch: string) =>
    run(`Fast-forward ${branch} → ${remote}/${remoteBranch}`, () =>
      api.fastForwardBranch(appState.repo, branch, remote, remoteBranch),
    ),
  createTag: (name: string, target: string, message?: string) =>
    run(`Create tag ${name}`, () => api.createTag(appState.repo, name, target, message)),
  deleteTag: (name: string) =>
    run(`Delete tag ${name}`, () => api.deleteTag(appState.repo, name)),
  fetch: () => {
    if (appState.repoLoading) {
      appState.status = "Repository is still loading — try again in a moment.";
      return Promise.resolve(false);
    }
    return run("Fetch", () => api.fetch(appState.repo));
  },
  merge: (reference: string, opts?: { noFf?: boolean; squash?: boolean }) =>
    runOp(`Merge ${reference}`, () =>
      api.merge(appState.repo, reference, opts?.noFf ?? false, opts?.squash ?? false),
    ),
  cherryPick: (shas: string[], label?: string) =>
    runOp(label ?? "Cherry-pick", () => api.cherryPick(appState.repo, shas)),
  revert: (shas: string[], label?: string) =>
    runOp(label ?? "Revert", () => api.revert(appState.repo, shas)),
  opContinue: (kind: string) =>
    runOp(`Continue ${kind}`, () => api.opContinue(appState.repo, kind)),
  opSkip: (kind: string) => runOp("Skip commit", () => api.opSkip(appState.repo, kind)),
  opAbort: (kind: string) => run(`Abort ${kind}`, () => api.opAbort(appState.repo, kind)),
  resolveConflict: (path: string, ours: boolean) =>
    runResolve(`Resolve ${path}`, () => api.resolveConflict(appState.repo, path, ours)),
  resolveKeep: (path: string) =>
    runResolve(`Keep ${path}`, () => api.resolveKeep(appState.repo, path)),
  resolveRemove: (path: string) =>
    runResolve(`Remove ${path}`, () => api.resolveRemove(appState.repo, path)),
  reset: (target: string, mode: "soft" | "mixed" | "hard", consequence: string) =>
    runDestructive(
      `Reset (${mode})`,
      consequence,
      (backup) => api.reset(appState.repo, target, mode, backup),
    ),
  amend: (message: string | null, resetAuthorDate: boolean, resetCommitterDate: boolean) =>
    runDestructive(
      "Amend commit",
      "Rewrites the latest commit (its hash changes).",
      (backup) => api.amend(appState.repo, message, resetAuthorDate, resetCommitterDate, backup),
    ),
  rebaseOnto: (onto: string, consequence: string) =>
    runDestructiveRebase(
      `Rebase onto ${onto}`,
      consequence,
      (backup) => api.rebase(appState.repo, onto, backup),
    ),
  rebaseInteractive: (base: string, steps: RebaseStep[], consequence: string) =>
    runDestructiveRebase(
      "Interactive rebase",
      consequence,
      (backup) => api.rebaseInteractive(appState.repo, base, steps, backup),
    ),

  // ── Single-commit message editing (C3) ────────────────────────────────────
  // Read the FULL message (%B, subject + body) of one commit. Read-only; safe in
  // browser preview (returns "" with no repo / outside Tauri).
  getCommitMessage: (sha: string): Promise<string> => {
    if (!isTauri() || !appState.repo) return Promise.resolve("");
    return api.commitMessage(appState.repo, sha);
  },

  // Set a commit's message. HEAD → amend (no date reset); otherwise → reword via
  // an interactive rebase of base..HEAD (pick every commit, reword the target).
  // Both paths already route through the destructive machinery (auto-backup +
  // confirm dialog + graph reload), so no confirm/reload is added here.
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

  // ── Working-copy actions (Phase 5) ────────────────────────────────────────
  stage: (paths: string[]) =>
    runWorktree(`Stage ${paths.length} file(s)`, () => api.stage(appState.repo, paths)),
  unstage: (paths: string[]) =>
    runWorktree(`Unstage ${paths.length} file(s)`, () => api.unstage(appState.repo, paths)),
  stageHunk: (path: string, hunkIndex: number) =>
    runWorktree(`Stage hunk in ${path}`, () => api.stageHunk(appState.repo, path, hunkIndex, appState.effectiveDiffContext)),
  unstageHunk: (path: string, hunkIndex: number) =>
    runWorktree(`Unstage hunk in ${path}`, () => api.unstageHunk(appState.repo, path, hunkIndex, appState.effectiveDiffContext)),
  stageLines: (path: string, hunkIndex: number, selected: number[]) =>
    runWorktree(`Stage ${selected.length} line(s) in ${path}`, () => api.stageLines(appState.repo, path, hunkIndex, selected, appState.effectiveDiffContext)),
  unstageLines: (path: string, hunkIndex: number, selected: number[]) =>
    runWorktree(`Unstage ${selected.length} line(s) in ${path}`, () => api.unstageLines(appState.repo, path, hunkIndex, selected, appState.effectiveDiffContext)),
  // Hunk/line discard is DESTRUCTIVE and NOT undoable — same reasoning as the
  // file-level `discard` below: uncommitted work is not in the reflog, so there is
  // no backup bundle to take. Guard BEFORE confirming so the browser preview never
  // shows a dialog it cannot honour.
  discardHunk: async (path: string, hunkIndex: number, changedLines: number): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    // State the LINE COUNT, not "this hunk": in Whole-file mode git emits one hunk
    // spanning the entire file, so a hunk discard can revert every unstaged change in
    // it — a boundary the user cannot see from the dialog.
    const n = changedLines;
    const confirmed = await dialogs.confirm({
      title: "Discard hunk",
      message: `Permanently discard ${n} changed line${n === 1 ? "" : "s"} in ${path}. This cannot be undone. (Stash instead to keep them.)`,
      confirmLabel: "Discard",
      danger: true,
    });
    if (!confirmed) return false;
    return runWorktree(`Discard hunk in ${path}`, () =>
      api.discardHunk(appState.repo, path, hunkIndex, appState.effectiveDiffContext),
    );
  },
  discardLines: async (path: string, hunkIndex: number, selected: number[]): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    const n = selected.length;
    const confirmed = await dialogs.confirm({
      title: "Discard lines",
      message: `Permanently discard ${n} line${n === 1 ? "" : "s"} in ${path}. This cannot be undone. (Stash instead to keep them.)`,
      confirmLabel: "Discard",
      danger: true,
    });
    if (!confirmed) return false;
    return runWorktree(`Discard ${n} line(s) in ${path}`, () =>
      api.discardLines(appState.repo, path, hunkIndex, selected, appState.effectiveDiffContext),
    );
  },
  commitChanges: (message: string, signoff = false) =>
    runWorktree("Commit", () => api.commit(appState.repo, message, signoff)),
  // Seamless composer amend (Fork-style): folds the currently-staged changes into
  // HEAD with the (edited) message via `git commit --amend -F <file>`. Unlike the
  // `amend` action above it does NOT route through runDestructive's blocking confirm
  // dialog — Fork amends without a modal; the configurable auto-backup is the safety
  // net. resetAuthorDate/resetCommitterDate=false preserve the original author.
  amendCommit: async (message: string): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    try {
      appState.status = "Amend commit…";
      const res = await api.amend(
        appState.repo,
        message,
        false,
        false,
        appState.autoBackupDestructive,
      );
      // Capture the RewriteResult so the one-click Undo bar + backup log appear
      // (parity with the destructive `amend`; runWorktree would have dropped them).
      appState.setLastUndo(res.undo);
      if (res.bundle) appState.appendLog(`[backup] ${res.bundle}`);
      try {
        await refreshWorkingChanges();
      } catch (e) {
        console.warn("[gte] working changes refresh after amend failed", e);
      }
      try {
        await reloadGraph();
      } catch (e) {
        console.warn("[gte] graph refresh after amend failed", e);
      }
      appState.status = "Amend commit — done.";
      return true;
    } catch (e) {
      appState.status = `Amend commit failed: ${firstLine(e)}`;
      return false;
    }
  },
  stashPush: (message: string | null) =>
    runWorktree("Stash changes", () => api.stashPush(appState.repo, message)),
  stashApply: (index: number) =>
    runWorktree(`Apply stash@{${index}}`, () => api.stashApply(appState.repo, index)),
  stashPop: (index: number) =>
    runWorktree(`Pop stash@{${index}}`, () => api.stashPop(appState.repo, index)),
  stashDrop: (index: number) =>
    runWorktree(`Drop stash@{${index}}`, () => api.stashDrop(appState.repo, index)),

  // Discard (tracked) and clean (untracked) are DESTRUCTIVE and NOT undoable.
  // Route through a plain danger confirm — no backup bundle (uncommitted work
  // is not in reflog; a --all bundle wouldn't capture it either). Offer "Stash
  // instead" in the copy so the user knows the safe alternative.
  discard: async (paths: string[]): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    const n = paths.length;
    const confirmed = await dialogs.confirm({
      title: "Discard changes",
      message: `Permanently discard changes to ${n} file${n === 1 ? "" : "s"}. This cannot be undone. (Stash instead to keep them.)`,
      confirmLabel: "Discard",
      danger: true,
    });
    if (!confirmed) return false;
    return runWorktree(`Discard ${n} file(s)`, () => api.discard(appState.repo, paths));
  },

  clean: async (paths: string[]): Promise<boolean> => {
    if (!isTauri()) {
      appState.status = "That action needs the desktop app (not the browser preview).";
      return false;
    }
    if (!appState.repo) {
      appState.status = "Open a repository first.";
      return false;
    }
    const n = paths.length;
    const confirmed = await dialogs.confirm({
      title: "Remove untracked files",
      message: `Permanently discard changes to ${n} file${n === 1 ? "" : "s"}. This cannot be undone. (Stash instead to keep them.)`,
      confirmLabel: "Discard",
      danger: true,
    });
    if (!confirmed) return false;
    return runWorktree(`Clean ${n} file(s)`, () => api.clean(appState.repo, paths));
  },

  undo: () => {
    const u: UndoSnapshot | null = appState.lastUndo;
    if (!u) return Promise.resolve(false);
    // The snapshot reverses the LAST op on the branch it was taken on. If the user
    // has since checked out a different branch, restoring would move the wrong one
    // (a repo switch already clears lastUndo via the store's repo setter). Invalidate
    // rather than reset the wrong branch.
    const current = appState.refsByKind.local.find((r) => r.isHead)?.name ?? null;
    if (u.branch !== current) {
      appState.status = "Undo unavailable — the checked-out branch changed.";
      appState.setLastUndo(null);
      return Promise.resolve(false);
    }
    return run(`Undo ${u.label}`, () => api.undoOp(appState.repo, u.sha)).then((ok) => {
      if (ok) appState.setLastUndo(null);
      return ok;
    });
  },

  // ── Squash-merge (Phase 5 follow-up) ─────────────────────────────────────
  // A squash merge stages all changes but creates NO commit and leaves no
  // MERGE_HEAD, so the normal runOp/ConflictView flow does not apply. Instead:
  //   • clean squash  → reloadGraph + open composer with a prefilled message
  //   • conflict squash → same (working copy surfaces conflicts; user resolves then commits)
  squashMerge: async (reference: string): Promise<void> => {
    if (!isTauri() || !appState.repo) {
      appState.status = "Squash merge needs the desktop app and an open repository.";
      return;
    }
    if (appState.remoteOpActive || appState.isRewriting) return;
    appState.status = `Squash-merging ${reference}…`;
    try {
      const outcome = await api.merge(appState.repo, reference, false, true);
      await reloadGraph();
      appState.setWorkingCopySelected(true); // open the commit composer on the staged result
      appState.setSuggestedCommitMessage(`Squash merge '${reference}'`);
      appState.status = outcome.conflicted
        ? `Squash merge of ${reference} has conflicts — resolve the files below, then commit.`
        : `Squashed ${reference} — review the staged changes and commit.`;
    } catch (e) {
      appState.status = `Squash merge failed: ${firstLine(e)}`;
    }
  },

  // ── Remote actions (Phase 6) ───────────────────────────────────────────────
  pull: () => {
    if (appState.repoLoading) {
      appState.status = "Repository is still loading — try again in a moment.";
      return Promise.resolve(false);
    }
    const label = appState.pullRebase ? "Pull (rebase)" : "Pull";
    return runRemote(label, (onLine, creds) =>
      api.pull(appState.repo, appState.pullRebase, onLine, creds),
    );
  },

  push: async (forceWithLease = false): Promise<boolean> => {
    // Don't act on a stale remote/branch during the brief repo-switch load window
    // (remotesState/refsDetailed are kept from the previous repo until reload).
    if (appState.repoLoading) {
      appState.status = "Repository is still loading — try again in a moment.";
      return false;
    }
    if (forceWithLease) {
      const ok = await dialogs.confirm({
        title: "Force push",
        message:
          "Force-push with lease? This overwrites the remote branch if it matches your last fetch. Any commits others pushed since your last fetch will be lost.",
        confirmLabel: "Force push",
        danger: true,
      });
      if (!ok) return false;
    }

    // Determine the current branch + whether it already has an upstream tracking ref.
    const branch = appState.refsByKind.local.find((r) => r.isHead)?.name ?? null;
    const hasUpstream = branch
      ? (appState.refsDetailed.find((r) => r.kind === "local" && r.name === branch)?.upstream ??
          null) !== null
      : false;
    // Use the first configured remote, or "origin" as fallback.
    const remote = appState.remotes[0]?.name ?? "origin";
    const label = forceWithLease ? "Force push" : "Push";

    return runRemote(label, (onLine, creds) =>
      api.push(appState.repo, remote, branch, forceWithLease, !hasUpstream, onLine, creds),
    );
  },

  cancelRemote: () => api.cancelRemote(),

  remoteAdd: (name: string, url: string) =>
    run(`Add remote ${name}`, () => api.remoteAdd(appState.repo, name, url)),

  remoteRemove: (name: string) =>
    run(`Remove remote ${name}`, () => api.remoteRemove(appState.repo, name)),

  remoteSetUrl: (name: string, url: string) =>
    run(`Set ${name} url`, () => api.remoteSetUrl(appState.repo, name, url)),

  createGithubRepo: (name: string, isPrivate: boolean, description: string) =>
    run(`Create GitHub repo ${name}`, async () => {
      await api.githubCreateRepo(appState.repo, name, isPrivate, description);
      await refreshRefs();
    }),

  // Jump to an EXISTING pull request on the GitHub screen (branch context menu,
  // title-bar PR chip). ensure() is awaited BEFORE selecting — a fresh GitHub-
  // screen mount runs ensure() itself, and a first load resets selectedItem;
  // awaiting it here makes the mount's ensure() an early-return no-op, so the
  // selection sticks (same proven sequence as createPullRequest's success path).
  openPrInApp: async (number: number): Promise<void> => {
    appState.setActiveView("github");
    await githubState.ensure(appState.repo);
    githubState.setActiveTab("pulls");
    githubState.openItem("pr", number);
  },

  // Check out a PR's head branch locally via `gh pr checkout` (handles fork
  // heads natively). run() already reloads the graph, which refreshes refs/
  // status/working copy — the same post-checkout refresh a branch checkout gets.
  checkoutPullRequest: (number: number) =>
    run(`Checkout PR #${number}`, () => api.githubPrCheckout(appState.repo, number)),

  // Create-PR wizard: prefill title/body from the branch's commits, collect
  // title/body/base/draft in a dialog, create via `gh` (auto-pushing the branch
  // first if needed), then jump to the new PR on the GitHub screen.
  createPullRequest: async () => {
    const branch = appState.refsByKind.local.find((r) => r.isHead)?.name ?? null;
    if (!branch) {
      appState.status = "No branch checked out — check out a branch to create a pull request.";
      return;
    }
    // Candidate base branches: every local branch except the current one, with
    // the repo's default branch first (from GitHub stats when loaded, else
    // main/master by convention).
    const others = appState.refsByKind.local.map((r) => r.name).filter((n) => n !== branch);
    if (others.length === 0) {
      appState.status = "No other local branch to use as the PR base.";
      return;
    }
    // Only trust stats' default branch when it was loaded for THIS repo — the
    // GitHub screen caches one repo's stats, which may belong to another tab.
    const statsDefault =
      githubState.loadedRepoPath === appState.repo ? githubState.stats?.defaultBranch : null;
    const preferred =
      [statsDefault, "main", "master"].find(
        (n): n is string => !!n && others.includes(n),
      ) ?? null;
    const bases = preferred ? [preferred, ...others.filter((n) => n !== preferred)] : others;
    // Best-effort prefill from the commit subjects on base..HEAD.
    const subjects = await api
      .branchSubjects(appState.repo, bases[0], 20)
      .catch(() => [] as string[]);
    const prefill = prefillFromSubjects(subjects, branch);
    const v = await dialogs.openCreatePr(branch, bases, prefill.title, prefill.body);
    if (!v) return;
    await run("Create pull request", async () => {
      const number = await api.githubPrCreate(appState.repo, v.title, v.body, v.base, v.draft);
      // The auto-push may have just created the branch's upstream.
      await refreshRefs();
      // Jump straight to the new PR on the GitHub screen. Await ensure() BEFORE
      // selecting: a fresh GitHub-screen mount runs ensure() itself, and a first
      // load (loadedRepo mismatch) resets selectedItem — awaiting it here makes
      // the mount's ensure() an early-return no-op, so the selection sticks.
      appState.setActiveView("github");
      await githubState.ensure(appState.repo);
      githubState.setActiveTab("pulls");
      githubState.bumpReload();
      githubState.openItem("pr", number);
    });
  },
};
