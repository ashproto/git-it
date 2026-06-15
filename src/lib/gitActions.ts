// Action layer over the Tauri op commands: each action guards for the desktop
// app, runs the op, refreshes the graph, and reports status — so components just
// call gitActions.checkout(...) etc. without repeating that plumbing.
import { appState } from "./store.svelte";
import { api } from "./api";
import { SAMPLE_GRAPH } from "./graph/sample";
import type { OpOutcome, RemoteOutcome, RewriteResult, RebaseOutcome, RebaseStep, UndoSnapshot } from "./types";
import { dialogs } from "./dialogs.svelte";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const PAGE = 150;

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

// Refresh the detailed ref list (ahead/behind/upstream) and the remotes list.
// Tauri-only; silently no-ops in browser preview.
export async function refreshRefs(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  try {
    appState.setRefsDetailed(await api.listRefs(appState.repo));
    appState.setRemotes(await api.remotes(appState.repo));
  } catch (e) {
    console.warn("[gte] refs/remotes refresh failed", e);
  }
}

export async function reloadGraph(): Promise<void> {
  if (!isTauri()) {
    appState.setGraphCommits(SAMPLE_GRAPH);
    appState.setGraphHasMore(false);
    return;
  }
  if (!appState.repo) return;
  const gc = await api.loadGraph(appState.repo, PAGE, 0);
  appState.setGraphCommits(gc);
  appState.setGraphHasMore(gc.length === PAGE);
  await refreshStatus();
  await refreshWorkingChanges();
  await refreshRefs();
}

export async function loadMoreGraph(): Promise<void> {
  if (!isTauri() || !appState.repo) return;
  if (!appState.graphHasMore || appState.graphLoadingMore) return;
  appState.setGraphLoadingMore(true);
  try {
    const gc = await api.loadGraph(appState.repo, PAGE, appState.graphCommits.length);
    appState.appendGraphCommits(gc);
    appState.setGraphHasMore(gc.length === PAGE);
  } catch (e) {
    console.warn("[gte] load more failed", e);
  } finally {
    appState.setGraphLoadingMore(false);
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
  try {
    appState.status = `${label}…`;
    await fn();
    // A failed graph refresh shouldn't make a successful op look failed.
    try {
      await reloadGraph();
    } catch (e) {
      console.warn("[gte] graph refresh after op failed", e);
    }
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    appState.status = `${label} failed: ${firstLine(e)}`;
    return false;
  }
}

// Git error/output text is often multiline; toasts get only the first line (contract d).
function firstLine(e: unknown): string {
  return String(e).split("\n")[0];
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
    // Refresh working changes first (fast), then the full graph.
    try {
      await refreshWorkingChanges();
    } catch (e) {
      console.warn("[gte] working changes refresh after op failed", e);
    }
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
  deleteBranch: (name: string, force: boolean) =>
    run(`Delete branch ${name}`, () => api.deleteBranch(appState.repo, name, force)),
  createTag: (name: string, target: string, message?: string) =>
    run(`Create tag ${name}`, () => api.createTag(appState.repo, name, target, message)),
  deleteTag: (name: string) =>
    run(`Delete tag ${name}`, () => api.deleteTag(appState.repo, name)),
  fetch: () => run("Fetch", () => api.fetch(appState.repo)),
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
  // ── Working-copy actions (Phase 5) ────────────────────────────────────────
  stage: (paths: string[]) =>
    runWorktree(`Stage ${paths.length} file(s)`, () => api.stage(appState.repo, paths)),
  unstage: (paths: string[]) =>
    runWorktree(`Unstage ${paths.length} file(s)`, () => api.unstage(appState.repo, paths)),
  stageHunk: (path: string, hunkIndex: number) =>
    runWorktree(`Stage hunk in ${path}`, () => api.stageHunk(appState.repo, path, hunkIndex)),
  unstageHunk: (path: string, hunkIndex: number) =>
    runWorktree(`Unstage hunk in ${path}`, () => api.unstageHunk(appState.repo, path, hunkIndex)),
  commitChanges: (message: string, signoff = false) =>
    runWorktree("Commit", () => api.commit(appState.repo, message, signoff)),
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

  // ── Remote actions (Phase 6) ───────────────────────────────────────────────
  pull: () => {
    const label = appState.pullRebase ? "Pull (rebase)" : "Pull";
    return runRemote(label, (onLine, creds) =>
      api.pull(appState.repo, appState.pullRebase, onLine, creds),
    );
  },

  push: async (forceWithLease = false): Promise<boolean> => {
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
};
