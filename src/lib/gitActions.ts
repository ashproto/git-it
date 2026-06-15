// Action layer over the Tauri op commands: each action guards for the desktop
// app, runs the op, refreshes the graph, and reports status — so components just
// call gitActions.checkout(...) etc. without repeating that plumbing.
import { appState } from "./store.svelte";
import { api } from "./api";
import { SAMPLE_GRAPH } from "./graph/sample";
import type { OpOutcome } from "./types";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const RELOAD_COUNT = 500;

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

export async function reloadGraph(): Promise<void> {
  if (!isTauri()) {
    appState.setGraphCommits(SAMPLE_GRAPH);
    return;
  }
  if (!appState.repo) return;
  const gc = await api.loadGraph(appState.repo, RELOAD_COUNT, 0);
  appState.setGraphCommits(gc);
  await refreshStatus();
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
    appState.status = `${label} failed: ${e}`;
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
};
