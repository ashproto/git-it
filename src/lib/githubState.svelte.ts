import { appState } from "./store.svelte";
import { api } from "./api";
import { parseGithubRemote } from "./github/remote";
import type {
  GhAvailability,
  GhRepoStats,
  GithubError,
  GhPull,
  GhIssue,
  GhRelease,
  GhRun,
  PullStateFilter,
  IssueStateFilter,
} from "./types";

export type GithubTab = "overview" | "pulls" | "issues" | "releases" | "actions" | "insights";

export type PanelStatus = "idle" | "loading" | "ok" | "error";

/** A reactive cache for one lazily-loaded panel. `load(key, fetcher)` is a no-op
 *  when the same `key` (repo + filter + refresh-nonce) is already loaded/loading,
 *  and discards a stale in-flight result if a newer `key` superseded it. */
function makePanel<T>() {
  let status = $state<PanelStatus>("idle");
  let data = $state<T | null>(null);
  let error = $state<GithubError | null>(null);
  let key: string | null = null;
  return {
    get status() {
      return status;
    },
    get data() {
      return data;
    },
    get error() {
      return error;
    },
    reset() {
      key = null;
      status = "idle";
      data = null;
      error = null;
    },
    async load(k: string, fetcher: () => Promise<T>) {
      if (key === k && (status === "ok" || status === "loading")) return;
      key = k;
      status = "loading";
      error = null;
      try {
        const d = await fetcher();
        if (key !== k) return; // superseded by a newer load
        data = d;
        status = "ok";
      } catch (e) {
        if (key !== k) return;
        error = e as GithubError;
        status = "error";
      }
    },
  };
}

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function makeGithubState() {
  let availability = $state<GhAvailability | null>(null);
  let availLoading = $state(false);
  let stats = $state<GhRepoStats | null>(null);
  let statsError = $state<GithubError | null>(null);
  let statsLoading = $state(false);
  let activeTab = $state<GithubTab>("overview");
  let loadedRepo: string | null = null;

  const pulls = makePanel<GhPull[]>();
  const issues = makePanel<GhIssue[]>();
  const releases = makePanel<GhRelease[]>();
  const runs = makePanel<GhRun[]>();
  let pullState = $state<PullStateFilter>("open");
  let issueState = $state<IssueStateFilter>("open");
  let reloadNonce = $state(0);

  function loadPulls(repo: string) {
    return pulls.load(`${repo}|${pullState}|${reloadNonce}`, () =>
      api.githubPulls(repo, pullState, 50),
    );
  }
  function loadIssues(repo: string) {
    return issues.load(`${repo}|${issueState}|${reloadNonce}`, () =>
      api.githubIssues(repo, issueState, 50),
    );
  }
  function loadReleases(repo: string) {
    return releases.load(`${repo}|${reloadNonce}`, () => api.githubReleases(repo));
  }
  function loadRuns(repo: string) {
    return runs.load(`${repo}|${reloadNonce}`, () => api.githubRuns(repo, 30));
  }

  async function loadStats(repo: string) {
    statsLoading = true;
    statsError = null;
    stats = null;
    try {
      stats = await api.githubRepoStats(repo);
    } catch (e) {
      statsError = e as GithubError;
    } finally {
      statsLoading = false;
    }
  }

  async function ensure(repo: string) {
    if (!isTauri()) return; // gh paths are desktop-only
    if (loadedRepo === repo && availability) return;
    loadedRepo = repo;
    pulls.reset();
    issues.reset();
    releases.reset();
    runs.reset();
    availability = null;
    stats = null;
    statsError = null;
    availLoading = true;
    try {
      availability = await api.githubAvailability(repo);
    } catch {
      availability = { kind: "NotInstalled" };
    } finally {
      availLoading = false;
    }
    if (availability && availability.kind === "Ok") {
      void loadStats(repo);
    }
  }

  return {
    get availability() {
      return availability;
    },
    get availLoading() {
      return availLoading;
    },
    get stats() {
      return stats;
    },
    get statsError() {
      return statsError;
    },
    get statsLoading() {
      return statsLoading;
    },
    get activeTab() {
      return activeTab;
    },
    /** Cheap, no-`gh` check used for sidebar nav visibility. */
    get hasGithubRemote(): boolean {
      return appState.remotes.some((r) => parseGithubRemote(r.url) !== null);
    },
    setActiveTab(t: GithubTab) {
      activeTab = t;
    },
    get pulls() {
      return pulls;
    },
    get issues() {
      return issues;
    },
    get releases() {
      return releases;
    },
    get runs() {
      return runs;
    },
    get pullState() {
      return pullState;
    },
    setPullState(s: PullStateFilter) {
      pullState = s;
    },
    get issueState() {
      return issueState;
    },
    setIssueState(s: IssueStateFilter) {
      issueState = s;
    },
    /** Read by each tab's load-effect so a Refresh re-runs them. */
    get reloadNonce() {
      return reloadNonce;
    },
    loadPulls,
    loadIssues,
    loadReleases,
    loadRuns,
    ensure,
    refresh(repo: string) {
      loadedRepo = null;
      reloadNonce++;
      return ensure(repo);
    },
  };
}

export const githubState = makeGithubState();
