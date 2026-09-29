import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { dialogs } from "./dialogs.svelte";
import { errors } from "./errors.svelte";
import { gitActions, reloadGraph } from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";
import { appState } from "./store.svelte";
import type { Ref, RemoteOutcome, RepoStatus } from "./types";

// Setting appState.repo under the stubbed Tauri window persists lastActiveRepo via the real plugin, which rejects and warns in Node; this stand-in keeps output clean.
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));

const REPO = "/tmp/git-it-error-test";

const repoStatus: RepoStatus = {
  head: { sha: SAMPLE_GRAPH[0].sha, branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};

// The checked-out main tracks an upstream that exists. Pull's pre-retry check only lets a
// sign-in retry through on such a branch, and every reload must keep it that way.
const sha = SAMPLE_GRAPH[0].sha;
const refs: Ref[] = [
  { name: "main", kind: "local", target_sha: sha, upstream: "origin/main", ahead: 0, behind: 0 },
  { name: "origin/main", kind: "remote", target_sha: sha, upstream: null, ahead: 0, behind: 0 },
];

const DIVERGENT = [
  "From /tmp/origin",
  "   d728e86..b362e35  main       -> origin/main",
  "hint: You have divergent branches and need to specify how to reconcile them.",
  "fatal: Need to specify how to reconcile divergent branches.",
].join("\n");

function outcome(patch: Partial<RemoteOutcome>): RemoteOutcome {
  return { ok: false, authFailed: false, conflicted: false, message: "", ...patch };
}

beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = REPO;
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH);
  // These are runRemote cases: pull's own options must never wait on the pull dialog.
  appState.setPullStrategy("merge");
  appState.setPullAutostash(false);
  appState.setPullAskEveryTime(false);
  appState.setRepoStatus(repoStatus);
  appState.setRefsDetailed(refs);
  vi.spyOn(api, "loadGraph").mockResolvedValue(SAMPLE_GRAPH);
  vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue(refs);
  vi.spyOn(api, "remotes").mockResolvedValue([]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
});

afterEach(() => {
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
  errors.reset();
  appState.repo = "";
  appState.status = "Ready";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("wrapper failures open the error popup", () => {
  it("run: checkout", async () => {
    vi.spyOn(api, "checkout").mockRejectedValue(
      "error: pathspec 'nope' did not match any file(s) known to git",
    );
    expect(await gitActions.checkout("nope")).toBe(false);
    expect(errors.current).toMatchObject({
      kind: "error",
      title: "Checkout nope failed",
      headline: "pathspec 'nope' did not match any file(s) known to git",
    });
    expect(appState.status).toBe(
      "Checkout nope failed: pathspec 'nope' did not match any file(s) known to git",
    );
    expect(appState.statusIsError).toBe(true);
  });

  it("runOp: merge", async () => {
    vi.spyOn(api, "merge").mockRejectedValue("merge: nope - not something we can merge");
    expect(await gitActions.merge("nope")).toBeNull();
    expect(errors.current).toMatchObject({
      title: "Merge nope failed",
      headline: "merge: nope - not something we can merge",
    });
  });

  it("runResolve: resolve a conflict", async () => {
    vi.spyOn(api, "resolveConflict").mockRejectedValue("error: path 'f' does not have our version");
    expect(await gitActions.resolveConflict("f", true)).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Resolve f failed",
      headline: "path 'f' does not have our version",
    });
  });

  it("runWorktree: apply a stash", async () => {
    vi.spyOn(api, "stashApply").mockRejectedValue(
      "error: Your local changes to the following files would be overwritten by merge:\n\tf\nAborting",
    );
    expect(await gitActions.stashApply(0)).toBe(false);
    expect(errors.current?.title).toBe("Apply stash@{0} failed");
    expect(errors.current?.headline).toBe(
      "Your local changes to the following files would be overwritten by merge:\n\tf",
    );
  });

  it("runDestructive: reset", async () => {
    vi.spyOn(dialogs, "confirmDestructive").mockResolvedValue({ confirmed: true, backup: false });
    vi.spyOn(api, "reset").mockRejectedValue("fatal: ambiguous argument 'abc': unknown revision");
    expect(await gitActions.reset("abc", "hard", "Moves main.")).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Reset (hard) failed",
      headline: "ambiguous argument 'abc': unknown revision",
    });
  });

  it("runDestructiveRebase: rebase onto", async () => {
    vi.spyOn(dialogs, "confirmDestructive").mockResolvedValue({ confirmed: true, backup: false });
    vi.spyOn(api, "rebase").mockRejectedValue(
      "error: cannot rebase: You have unstaged changes.\nerror: Please commit or stash them.",
    );
    expect(await gitActions.rebaseOnto("main", "Rewrites commits.")).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Rebase onto main failed",
      headline: "cannot rebase: You have unstaged changes.",
    });
  });

  it("amendCommit", async () => {
    vi.spyOn(api, "amend").mockRejectedValue(
      "fatal: You are in the middle of a merge -- cannot amend.",
    );
    expect(await gitActions.amendCommit("message")).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Amend commit failed",
      headline: "You are in the middle of a merge -- cannot amend.",
    });
  });

  it("squashMerge", async () => {
    vi.spyOn(api, "merge").mockRejectedValue("fatal: refusing to merge unrelated histories");
    await gitActions.squashMerge("feature");
    expect(errors.current).toMatchObject({
      title: "Squash merge failed",
      headline: "refusing to merge unrelated histories",
    });
  });

  it("reloadGraph: a repository that can't be opened", async () => {
    vi.spyOn(api, "loadGraph").mockRejectedValue(
      "Failed to spawn command: No such file or directory (os error 2)",
    );
    await reloadGraph();
    expect(errors.current).toMatchObject({
      title: "Couldn't open git-it-error-test",
      headline: "Failed to spawn command: No such file or directory (os error 2)",
    });
    expect(errors.current?.output).toContain(REPO);
  });
});

describe("runRemote reports only failures the user didn't choose", () => {
  it("finishes a successful pull without an error popup", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ ok: true }));
    expect(await gitActions.pull()).toBe(true);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull — done.");
  });

  it("reports a failed pull with git's real error", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ message: DIVERGENT }));
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current).toMatchObject({
      title: "Pull failed",
      headline: "Need to specify how to reconcile divergent branches.",
    });
    expect(appState.status).toBe("Pull failed: Need to specify how to reconcile divergent branches.");
  });

  it("reports a failed pull when Cancel is pressed during the refresh after git finished", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ message: DIVERGENT }));
    vi.spyOn(api, "cancelRemote").mockResolvedValue(undefined);
    vi.mocked(api.loadGraph).mockImplementation(async () => {
      void gitActions.cancelRemote();
      return SAMPLE_GRAPH;
    });
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current?.title).toBe("Pull failed");
  });

  it("reports a thrown error", async () => {
    vi.spyOn(api, "pull").mockRejectedValue("A remote operation is already in progress.");
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current?.headline).toBe("A remote operation is already in progress.");
  });

  it("leaves conflicts to the conflict panel", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ conflicted: true, message: "CONFLICT" }));
    await gitActions.pull();
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull: conflicts to resolve.");
  });

  it("treats a pull the user cancelled as cancelled", async () => {
    let finish: (o: RemoteOutcome) => void = () => {};
    vi.spyOn(api, "pull").mockImplementation(
      () => new Promise<RemoteOutcome>((resolve) => (finish = resolve)),
    );
    vi.spyOn(api, "cancelRemote").mockResolvedValue(undefined);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(api.pull).toHaveBeenCalled());
    void gitActions.cancelRemote();
    finish(outcome({ message: "error: RPC failed; curl 56 Recv failure" }));
    expect(await pulling).toBe(false);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull cancelled.");
  });

  it("treats a rejected pull the user cancelled as cancelled", async () => {
    let fail: (error: Error) => void = () => {};
    vi.spyOn(api, "pull").mockImplementation(
      () => new Promise<RemoteOutcome>((_resolve, reject) => (fail = reject)),
    );
    vi.spyOn(api, "cancelRemote").mockResolvedValue(undefined);
    const pulling = gitActions.pull();
    await vi.waitFor(() => expect(api.pull).toHaveBeenCalled());
    void gitActions.cancelRemote();
    fail(new Error("killed"));
    expect(await pulling).toBe(false);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull cancelled.");
  });

  it("does not let a stray Cancel hide the next pull's failure", async () => {
    vi.spyOn(api, "cancelRemote").mockResolvedValue(undefined);
    await gitActions.cancelRemote();
    vi.spyOn(api, "pull").mockResolvedValue(outcome({ message: DIVERGENT }));
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current?.title).toBe("Pull failed");
  });

  it("does not report a dismissed sign-in", async () => {
    vi.spyOn(api, "pull").mockResolvedValue(
      outcome({ authFailed: true, message: "fatal: Authentication failed" }),
    );
    vi.spyOn(dialogs, "confirmCredentials").mockResolvedValue(null);
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Pull: sign-in cancelled.");
  });

  it("reports a retry that fails after signing in", async () => {
    vi.spyOn(api, "pull")
      .mockResolvedValueOnce(outcome({ authFailed: true, message: "fatal: Authentication failed" }))
      .mockResolvedValueOnce(
        outcome({
          authFailed: true,
          message:
            "remote: Invalid username or token.\nfatal: Authentication failed for 'https://github.com/o/r.git/'",
        }),
      );
    vi.spyOn(dialogs, "confirmCredentials").mockResolvedValue({ username: "u", password: "p" });
    expect(await gitActions.pull()).toBe(false);
    expect(errors.current?.headline).toBe(
      "Authentication failed for 'https://github.com/o/r.git/'",
    );
  });

  it("does not report a stale auth failure after the repository changed", async () => {
    const pull = vi
      .spyOn(api, "pull")
      .mockResolvedValue(outcome({ authFailed: true, message: "fatal: Authentication failed" }));
    vi.spyOn(dialogs, "confirmCredentials").mockImplementation(async () => {
      appState.repo = "/tmp/git-it-other-repo";
      return { username: "u", password: "p" };
    });
    expect(await gitActions.pull()).toBe(false);
    expect(pull).toHaveBeenCalledTimes(1);
    expect(errors.current).toBeNull();
    expect(appState.status).toBe("Repository changed while that dialog was open — nothing was done.");
  });
});
