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
