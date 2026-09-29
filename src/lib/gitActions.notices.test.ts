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
