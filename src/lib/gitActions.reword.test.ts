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
