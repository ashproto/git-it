import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { dialogs } from "./dialogs.svelte";
import {
  gitActions,
  loadMoreGraph,
  refreshActiveRepo,
  refreshRefs,
  reloadGraph,
} from "./gitActions";
import { SAMPLE_GRAPH } from "./graph/sample";
import { appState } from "./store.svelte";
import type { GraphCommit, Ref, RepoStatus } from "./types";

const repoStatus: RepoStatus = {
  head: { sha: SAMPLE_GRAPH[0].sha, branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};

const mainRef: Ref = {
  name: "main",
  kind: "local",
  target_sha: SAMPLE_GRAPH[0].sha,
  upstream: null,
  ahead: 0,
  behind: 0,
};

function graphWithoutFeatureDecoration(): GraphCommit[] {
  return SAMPLE_GRAPH.map((commit) => ({
    ...commit,
    refs: commit.refs.filter((ref) => ref.name !== "feature/graph-view"),
  }));
}

function mockRefreshApis(refreshedGraph: GraphCommit[]) {
  vi.spyOn(api, "loadGraph").mockResolvedValue(refreshedGraph);
  vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
  vi.spyOn(api, "workingChanges").mockResolvedValue([]);
  vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
  vi.spyOn(api, "remotes").mockResolvedValue([]);
  vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
}

beforeEach(() => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  appState.repo = "/tmp/git-it-partial-delete-test";
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH);
  appState.setGraphHasMore(true);
  appState.setRefsDetailed([
    mainRef,
    {
      name: "feature/graph-view",
      kind: "local",
      target_sha: SAMPLE_GRAPH[2].sha,
      upstream: null,
      ahead: 0,
      behind: 0,
    },
  ]);
  appState.setCurrent(SAMPLE_GRAPH[2].sha);
  appState.selected = new Set([SAMPLE_GRAPH[2].sha]);
  appState.setNewDate(SAMPLE_GRAPH[2].sha, new Date("2020-01-01T00:00:00Z"));
});

afterEach(() => {
  dialogs.resolveAlert();
  appState.repo = "";
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("partial branch-delete refresh", () => {
  it("removes stale graph decorations without discarding queued edits", async () => {
    const refreshedGraph = graphWithoutFeatureDecoration();
    mockRefreshApis(refreshedGraph);
    vi.spyOn(api, "deleteBranch").mockRejectedValue(
      new Error("Deleted local branch, but remote delete failed"),
    );

    const ok = await gitActions.deleteBranch(
      "feature/graph-view",
      false,
      true,
      "origin",
      "feature/graph-view",
    );

    expect(ok).toBe(false);
    expect(
      appState.graphCommits.some((commit) =>
        commit.refs.some((ref) => ref.name === "feature/graph-view"),
      ),
    ).toBe(false);
    expect(appState.refsByKind.local.some((ref) => ref.name === "feature/graph-view")).toBe(
      false,
    );
    expect(appState.currentSha).toBe(SAMPLE_GRAPH[2].sha);
    expect(appState.selected.has(SAMPLE_GRAPH[2].sha)).toBe(true);
    expect(appState.newDates.has(SAMPLE_GRAPH[2].sha)).toBe(true);
  });

  it("discards an in-flight stale page before refreshing the failed operation", async () => {
    let resolvePage: (commits: GraphCommit[]) => void = () => {};
    const page = new Promise<GraphCommit[]>((resolve) => {
      resolvePage = resolve;
    });
    let resolveRefresh: (commits: GraphCommit[]) => void = () => {};
    const refresh = new Promise<GraphCommit[]>((resolve) => {
      resolveRefresh = resolve;
    });
    const refreshedGraph = graphWithoutFeatureDecoration();
    const loadGraph = vi
      .spyOn(api, "loadGraph")
      .mockImplementationOnce(() => page)
      .mockImplementationOnce(() => refresh);
    vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
    vi.spyOn(api, "workingChanges").mockResolvedValue([]);
    vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
    vi.spyOn(api, "deleteBranch").mockRejectedValue(
      new Error("Deleted local branch, but remote delete failed"),
    );

    const loading = loadMoreGraph();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(1));
    const deleting = gitActions.deleteBranch(
      "feature/graph-view",
      false,
      true,
      "origin",
      "feature/graph-view",
    );
    resolvePage([
      {
        ...SAMPLE_GRAPH[2],
        sha: "stale-page-commit",
        refs: [{ name: "feature/graph-view", kind: "local", is_head: false }],
      },
    ]);

    await loading;
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(2));

    // The stale page must be rejected as soon as it resolves, not merely
    // overwritten later when the post-failure snapshot finishes.
    expect(appState.graphCommits.some((commit) => commit.sha === "stale-page-commit")).toBe(false);

    resolveRefresh(refreshedGraph);
    await deleting;

    expect(loadGraph).toHaveBeenCalledTimes(2);
    expect(appState.graphCommits).toEqual(refreshedGraph);
    expect(appState.graphCommits.some((commit) => commit.sha === "stale-page-commit")).toBe(false);
  });

  it("invalidates a stale page when an ordinary reload replaces the same-length graph", async () => {
    let resolvePage: (commits: GraphCommit[]) => void = () => {};
    const page = new Promise<GraphCommit[]>((resolve) => {
      resolvePage = resolve;
    });
    let resolveReload: (commits: GraphCommit[]) => void = () => {};
    const reload = new Promise<GraphCommit[]>((resolve) => {
      resolveReload = resolve;
    });
    const refreshedGraph = graphWithoutFeatureDecoration();
    const loadGraph = vi
      .spyOn(api, "loadGraph")
      .mockImplementationOnce(() => page)
      .mockImplementationOnce(() => reload);
    vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
    vi.spyOn(api, "workingChanges").mockResolvedValue([]);
    vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);

    const loading = loadMoreGraph();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(1));
    const reloading = reloadGraph();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(2));
    resolveReload(refreshedGraph);
    await reloading;

    resolvePage([
      {
        ...SAMPLE_GRAPH[2],
        sha: "stale-page-after-reload",
        refs: [{ name: "feature/graph-view", kind: "local", is_head: false }],
      },
    ]);
    await loading;

    expect(appState.graphCommits).toEqual(refreshedGraph);
    expect(appState.graphCommits.some((commit) => commit.sha === "stale-page-after-reload")).toBe(
      false,
    );
  });

  it("queues a watcher refresh that arrives while another preserving refresh is active", async () => {
    let resolveFirstRefresh: (commits: GraphCommit[]) => void = () => {};
    const firstRefresh = new Promise<GraphCommit[]>((resolve) => {
      resolveFirstRefresh = resolve;
    });
    let resolveQueuedRefresh: (commits: GraphCommit[]) => void = () => {};
    const queuedRefresh = new Promise<GraphCommit[]>((resolve) => {
      resolveQueuedRefresh = resolve;
    });
    const refreshedGraph = graphWithoutFeatureDecoration();
    const loadGraph = vi
      .spyOn(api, "loadGraph")
      .mockImplementationOnce(() => firstRefresh)
      .mockImplementationOnce(() => queuedRefresh);
    vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
    vi.spyOn(api, "workingChanges").mockResolvedValue([]);
    vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);

    refreshActiveRepo();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(1));
    refreshActiveRepo();
    // Let the second debounced request observe the still-active first refresh.
    await new Promise((resolve) => setTimeout(resolve, 160));
    expect(loadGraph).toHaveBeenCalledTimes(1);

    resolveFirstRefresh(SAMPLE_GRAPH);
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(2));
    resolveQueuedRefresh(refreshedGraph);

    await vi.waitFor(() => expect(appState.graphCommits).toEqual(refreshedGraph));
  });

  it("queues a watcher refresh that arrives during pagination", async () => {
    let resolvePage: (commits: GraphCommit[]) => void = () => {};
    const page = new Promise<GraphCommit[]>((resolve) => {
      resolvePage = resolve;
    });
    let resolveQueuedRefresh: (commits: GraphCommit[]) => void = () => {};
    const queuedRefresh = new Promise<GraphCommit[]>((resolve) => {
      resolveQueuedRefresh = resolve;
    });
    const refreshedGraph = graphWithoutFeatureDecoration();
    const loadGraph = vi
      .spyOn(api, "loadGraph")
      .mockImplementationOnce(() => page)
      .mockImplementationOnce(() => queuedRefresh);
    vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
    vi.spyOn(api, "workingChanges").mockResolvedValue([]);
    vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);

    const loading = loadMoreGraph();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(1));
    refreshActiveRepo();
    // Let the debounced watcher request observe the active page load.
    await new Promise((resolve) => setTimeout(resolve, 160));
    expect(loadGraph).toHaveBeenCalledTimes(1);

    resolvePage([]);
    await loading;
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(2));
    resolveQueuedRefresh(refreshedGraph);

    await vi.waitFor(() => expect(appState.graphCommits).toEqual(refreshedGraph));
  });

  it("gives a full reload priority over a later preserving refresh", async () => {
    let resolveReload: (commits: GraphCommit[]) => void = () => {};
    const reload = new Promise<GraphCommit[]>((resolve) => {
      resolveReload = resolve;
    });
    const refreshedGraph = graphWithoutFeatureDecoration();
    const loadGraph = vi
      .spyOn(api, "loadGraph")
      .mockImplementationOnce(() => reload)
      .mockRejectedValueOnce(new Error("queued preserving refresh failed"));
    vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
    vi.spyOn(api, "workingChanges").mockResolvedValue([]);
    vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
    vi.spyOn(console, "warn").mockImplementation(() => {});
    appState.setRepoLoading(true);

    const reloading = reloadGraph();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(1));
    refreshActiveRepo();
    // A preserving load must not start early and invalidate the full reload.
    await new Promise((resolve) => setTimeout(resolve, 160));
    expect(loadGraph).toHaveBeenCalledTimes(1);

    resolveReload(refreshedGraph);
    await reloading;

    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(2));
    expect(appState.graphCommits).toEqual(refreshedGraph);
    expect(appState.repoLoading).toBe(false);
  });

  it("queues a post-failure snapshot behind an older live refresh", async () => {
    let resolveLiveRefresh: (commits: GraphCommit[]) => void = () => {};
    const liveRefresh = new Promise<GraphCommit[]>((resolve) => {
      resolveLiveRefresh = resolve;
    });
    const refreshedGraph = graphWithoutFeatureDecoration();
    const loadGraph = vi
      .spyOn(api, "loadGraph")
      .mockImplementationOnce(() => liveRefresh)
      .mockResolvedValueOnce(refreshedGraph);
    vi.spyOn(api, "repoStatus").mockResolvedValue(repoStatus);
    vi.spyOn(api, "workingChanges").mockResolvedValue([]);
    vi.spyOn(api, "listRefs").mockResolvedValue([mainRef]);
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
    vi.spyOn(api, "deleteBranch").mockRejectedValue(
      new Error("Deleted local branch, but remote delete failed"),
    );

    refreshActiveRepo();
    await vi.waitFor(() => expect(loadGraph).toHaveBeenCalledTimes(1));
    const deleting = gitActions.deleteBranch(
      "feature/graph-view",
      false,
      true,
      "origin",
      "feature/graph-view",
    );
    resolveLiveRefresh(SAMPLE_GRAPH);

    await deleting;

    expect(loadGraph).toHaveBeenCalledTimes(2);
    expect(appState.graphCommits).toEqual(refreshedGraph);
  });

  it("falls back to graph refs when the detailed inventory refresh fails", async () => {
    appState.setRefsDetailed([mainRef]);
    expect(appState.refsByKind.local.some((ref) => ref.name === "feature/graph-view")).toBe(
      false,
    );
    vi.spyOn(api, "listRefs").mockRejectedValue(new Error("list refs failed"));
    vi.spyOn(api, "remotes").mockResolvedValue([]);
    vi.spyOn(api, "listWorktrees").mockResolvedValue([]);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await refreshRefs();

    expect(appState.refsDetailed).toEqual([]);
    expect(appState.refsByKind.local.some((ref) => ref.name === "feature/graph-view")).toBe(true);
  });
});
