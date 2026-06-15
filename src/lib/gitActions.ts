// Action layer over the Tauri op commands: each action guards for the desktop
// app, runs the op, refreshes the graph, and reports status — so components just
// call gitActions.checkout(...) etc. without repeating that plumbing.
import { appState } from "./store.svelte";
import { api } from "./api";
import { SAMPLE_GRAPH } from "./graph/sample";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const RELOAD_COUNT = 500;

export async function reloadGraph(): Promise<void> {
  if (!isTauri()) {
    appState.setGraphCommits(SAMPLE_GRAPH);
    return;
  }
  if (!appState.repo) return;
  const gc = await api.loadGraph(appState.repo, RELOAD_COUNT, 0);
  appState.setGraphCommits(gc);
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
    await reloadGraph();
    appState.status = `${label} — done.`;
    return true;
  } catch (e) {
    appState.status = `${label} failed: ${e}`;
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
};
