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
// The upstream's own ref: without it in the inventory, pullContext reads origin/main as gone.
const originMainRef: Ref = {
  name: "origin/main",
  kind: "remote",
  target_sha: "e1f2a3b4c5d6",
  upstream: null,
  ahead: 0,
  behind: 0,
};

beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = REPO;
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH);
  appState.setRefsDetailed([mainRef, originMainRef]);
  // The dialog reads the checked-out branch from the repository status, never the graph.
  appState.setRepoStatus(repoStatus);
  appState.setPullStrategy("merge");
  appState.setPullAutostash(false);
  appState.setPullAskEveryTime(false);
  vi.spyOn(api, "pull").mockResolvedValue(pulled);
  // runRemote reloads the graph after the op.
  vi.spyOn(api, "loadGraph").mockResolvedValue(SAMPLE_GRAPH);
  vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue([mainRef, originMainRef]);
  vi.spyOn(api, "remotes").mockResolvedValue([{ name: "origin", url: "https://example.com/x.git" }]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
});

afterEach(() => {
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
  if (dialogs.state.kind === "credentials") dialogs.resolveCredentials(null);
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
    expect(dialogs.state.kind).toBe("none");
    expect(api.pull).toHaveBeenCalledWith(REPO, "ffOnly", true, expect.any(Function), undefined);
    expect(appState.status).toBe("Pull (fast-forward only) — done.");
  });

  it("asks first when the setting is on, saves the choice as the new default, then pulls with it", async () => {
    appState.setPullAskEveryTime(true);

    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    if (dialogs.state.kind !== "pull") throw new Error("pull dialog did not open");
    expect(dialogs.state.branch).toBe("main");
    expect(dialogs.state.upstream).toBe("origin/main");
    expect(dialogs.state.ahead).toBe(2);
    expect(dialogs.state.behind).toBe(3);
    expect(dialogs.state.strategy).toBe("merge");
    expect(dialogs.state.autostash).toBe(false);
    expect(api.pull).not.toHaveBeenCalled();

    dialogs.setPullStrategy("rebase");
    dialogs.setPullAutostash(true);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "rebase", true, expect.any(Function), undefined);
    expect(appState.pullStrategy).toBe("rebase");
    expect(appState.pullAutostash).toBe(true);
    expect(appState.pullAskEveryTime).toBe(true);
    expect(appState.status).toBe("Pull (rebase) — done.");
  });

  it("shows the options on alt-click even when asking is off", async () => {
    const pulling = gitActions.pull({ showOptions: true });
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "merge", false, expect.any(Function), undefined);
  });

  it("turns asking off when 'Don't ask again' is ticked", async () => {
    appState.setPullAskEveryTime(true);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    dialogs.setPullDontAsk(true);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(true);
    expect(appState.pullAskEveryTime).toBe(false);
    expect(api.pull).toHaveBeenCalledTimes(1);
  });

  it("pulls nothing and keeps the defaults when the dialog is cancelled", async () => {
    appState.setPullAskEveryTime(true);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    dialogs.setPullStrategy("ffOnly");
    dialogs.resolvePull(false);

    await expect(pulling).resolves.toBe(false);
    expect(api.pull).not.toHaveBeenCalled();
    expect(appState.pullStrategy).toBe("merge");
  });

  it("reads the checked-out branch from the repository status, not the loaded graph page", async () => {
    // HEAD's commit is outside the loaded page, so no decoration carries is_head.
    appState.setGraphCommits(SAMPLE_GRAPH.slice(1));
    expect(appState.refsByKind.local.some((r) => r.isHead)).toBe(false);
    appState.setPullAskEveryTime(true);

    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    if (dialogs.state.kind !== "pull") throw new Error("pull dialog did not open");
    expect(dialogs.state.branch).toBe("main");
    expect(dialogs.state.upstream).toBe("origin/main");
    expect(dialogs.state.ahead).toBe(2);
    expect(dialogs.state.behind).toBe(3);

    dialogs.resolvePull(false);
    await expect(pulling).resolves.toBe(false);
  });

  it("reads preferences only after the store has hydrated", async () => {
    let release: () => void = () => {};
    const hydration = new Promise<void>((resolve) => {
      release = resolve;
    });
    vi.spyOn(appState, "waitForPullSettingsHydration").mockReturnValue(hydration);

    const pulling = gitActions.pull();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(api.pull).not.toHaveBeenCalled();

    // The persisted preference lands, then the barrier lifts.
    appState.setPullStrategy("ffOnly");
    release();

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledWith(REPO, "ffOnly", false, expect.any(Function), undefined);
  });
});

describe("a confirmed pull is bound to what the dialog showed", () => {
  // Opens the dialog, then watches every preference setter. The returned assertion
  // proves the refusal wrote nothing and pulled nothing.
  async function openDialog() {
    appState.setPullAskEveryTime(true);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
    const writes = [
      vi.spyOn(appState, "setPullStrategy"),
      vi.spyOn(appState, "setPullAutostash"),
      vi.spyOn(appState, "setPullAskEveryTime"),
    ];
    return {
      pulling,
      expectNothingHappened() {
        expect(api.pull).not.toHaveBeenCalled();
        for (const write of writes) expect(write).not.toHaveBeenCalled();
        expect(appState.pullStrategy).toBe("merge");
        expect(appState.pullAutostash).toBe(false);
      },
    };
  }

  it("refuses when the repository changed while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.repo = "/tmp/git-it-pull-flow-other";
    dialogs.setPullStrategy("ffOnly");
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
  });

  it("refuses when the repository is loading again after an A → B → A switch", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.repo = "/tmp/git-it-pull-flow-other";
    appState.repo = REPO; // same identity as the dialog's, but loading again
    expect(appState.repoLoading).toBe(true);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't pull yet" });
  });

  it("refuses when the checked-out branch changed while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.setRepoStatus({ ...repoStatus, head: { ...repoStatus.head, branch: "feature" } });
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull",
      headline: "The checked-out branch changed while that dialog was open — nothing was done.",
    });
  });

  it("refuses when the branch's upstream changed while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    appState.setRefsDetailed([{ ...mainRef, upstream: "upstream/main" }]);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't pull" });
  });

  it("refuses when the upstream was pruned while the dialog was open", async () => {
    const { pulling, expectNothingHappened } = await openDialog();

    // A fetch pruned origin/main: main still names it, and its tracking now reads 0/0.
    appState.setRefsDetailed([{ ...mainRef, ahead: 0, behind: 0 }]);
    dialogs.resolvePull(true);

    await expect(pulling).resolves.toBe(false);
    expectNothingHappened();
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull",
      headline: "origin/main no longer exists as of the last fetch — there's nothing to pull.",
    });
  });
});

describe("a sign-in retry is bound to the same branch and upstream", () => {
  const creds = { username: "u", password: "p" };

  // Clicks Pull (confirming Rebase + autostash when the dialog asks), fails the first
  // attempt on authentication, and waits for the sign-in prompt.
  async function untilSignIn({ ask }: { ask: boolean }) {
    vi.mocked(api.pull).mockResolvedValueOnce({
      ok: false,
      authFailed: true,
      conflicted: false,
      message: "fatal: Authentication failed",
    });
    appState.setPullAskEveryTime(ask);
    const pulling = gitActions.pull();
    if (ask) {
      await vi.waitFor(() => expect(dialogs.state.kind).toBe("pull"));
      dialogs.setPullStrategy("rebase");
      dialogs.setPullAutostash(true);
      dialogs.resolvePull(true);
    }
    await vi.waitFor(() => expect(dialogs.state.kind).toBe("credentials"));
    return { pulling };
  }

  it("retries with the same strategy and autostash, and the credentials entered", async () => {
    const { pulling } = await untilSignIn({ ask: true });
    dialogs.resolveCredentials(creds);

    await expect(pulling).resolves.toBe(true);
    expect(api.pull).toHaveBeenCalledTimes(2);
    expect(api.pull).toHaveBeenNthCalledWith(1, REPO, "rebase", true, expect.any(Function), undefined);
    expect(api.pull).toHaveBeenNthCalledWith(2, REPO, "rebase", true, expect.any(Function), creds);
    expect(appState.status).toBe("Pull (rebase) — done.");
  });

  it("refuses to retry when the checked-out branch changed during sign-in", async () => {
    const { pulling } = await untilSignIn({ ask: true });
    appState.setRepoStatus({ ...repoStatus, head: { ...repoStatus.head, branch: "feature" } });
    dialogs.resolveCredentials(creds);

    await expect(pulling).resolves.toBe(false);
    expect(api.pull).toHaveBeenCalledTimes(1);
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull",
      headline: "The checked-out branch changed while you were signing in — nothing was pulled.",
    });
    // The stale authentication failure is not reported on top.
    expect(errors.pending).toBe(1);
    expect(errors.last).toBeNull();
    expect(appState.status).toBe(
      "The checked-out branch changed while you were signing in — nothing was pulled.",
    );
  });

  it("refuses to retry when the repository is loading again after sign-in", async () => {
    const { pulling } = await untilSignIn({ ask: true });
    appState.repo = "/tmp/git-it-pull-flow-other";
    appState.repo = REPO; // same identity, so runRemote's repository check passes
    dialogs.resolveCredentials(creds);

    await expect(pulling).resolves.toBe(false);
    expect(api.pull).toHaveBeenCalledTimes(1);
    expect(errors.current).toMatchObject({ kind: "notice", title: "Can't pull yet" });
    expect(errors.last).toBeNull();
  });

  it("refuses to retry after a branch change when asking is off, too", async () => {
    const { pulling } = await untilSignIn({ ask: false });
    appState.setRepoStatus({ ...repoStatus, head: { ...repoStatus.head, branch: "feature" } });
    dialogs.resolveCredentials(creds);

    await expect(pulling).resolves.toBe(false);
    expect(api.pull).toHaveBeenCalledTimes(1);
    expect(errors.current).toMatchObject({
      kind: "notice",
      title: "Can't pull",
      headline: "The checked-out branch changed while you were signing in — nothing was pulled.",
    });
    expect(errors.last).toBeNull();
  });
});
