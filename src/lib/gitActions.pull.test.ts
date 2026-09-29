import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { dialogs } from "./dialogs.svelte";
import { errors } from "./errors.svelte";
import { gitActions } from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";
import { appState } from "./store.svelte";
import type { Ref, RemoteOutcome, RepoStatus } from "./types";

// Once `window.__TAURI_INTERNALS__` is stubbed, the settings persist through the Tauri
// store plugin. There is no IPC under vitest, so give it an in-memory stand-in.
vi.mock("@tauri-apps/plugin-store", () => ({
  load: async () => ({
    get: async () => undefined,
    set: async () => {},
    save: async () => {},
  }),
}));

const REPO = "/tmp/git-it-pull-flow-test";
const pulled: RemoteOutcome = { ok: true, authFailed: false, conflicted: false, message: "" };
const repoStatus: RepoStatus = {
  head: { sha: SAMPLE_GRAPH[0].sha, branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};
// SAMPLE_GRAPH decorates its first commit with `main` as HEAD; this inventory entry
// gives that branch an upstream and ahead/behind counts.
const mainRef: Ref = {
  name: "main",
  kind: "local",
  target_sha: SAMPLE_GRAPH[0].sha,
  upstream: "origin/main",
  ahead: 2,
  behind: 3,
};

beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = REPO;
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH);
  appState.setRefsDetailed([mainRef]);
  appState.setPullStrategy("merge");
  appState.setPullAutostash(false);
  appState.setPullAskEveryTime(false);
  vi.spyOn(api, "pull").mockResolvedValue(pulled);
  // runRemote reloads the graph after the op.
  vi.spyOn(api, "loadGraph").mockResolvedValue(SAMPLE_GRAPH);
  vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
  vi.spyOn(api, "remotes").mockResolvedValue([{ name: "origin", url: "https://example.com/x.git" }]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
});

afterEach(() => {
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
  errors.reset();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("gitActions.pull", () => {
  it("refuses with a notice while the repository is still loading", async () => {
    appState.setRepoLoading(true);

    const ok = await gitActions.pull();

    expect(ok).toBe(false);
    expect(api.pull).not.toHaveBeenCalled();
    expect(appState.status).toBe("Repository is still loading — try again in a moment.");
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull yet",
      headline: "Repository is still loading — try again in a moment.",
    });
  });

  it("pulls with the saved strategy and autostash when asking is off", async () => {
    appState.setPullStrategy("ffOnly");
    appState.setPullAutostash(true);

    const ok = await gitActions.pull();

    expect(ok).toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "ffOnly", true, expect.any(Function), undefined);
    expect(appState.status).toBe("Pull (fast-forward only) — done.");
  });
});
