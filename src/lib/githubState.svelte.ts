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
  GhTraffic,
  GhContributor,
  GhActivity,
  GhMilestone,
  GhLabel,
  PullStateFilter,
  IssueStateFilter,
  GhPullDetail,
  GhIssueDetail,
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

  const traffic = makePanel<GhTraffic>();
  const contributors = makePanel<GhContributor[]>();
  const activity = makePanel<GhActivity>();
  const milestones = makePanel<GhMilestone[]>();
  const labels = makePanel<GhLabel[]>();

  const prDetail = makePanel<GhPullDetail>();
  const issueDetail = makePanel<GhIssueDetail>();
  let selectedItem = $state<{ kind: "pr" | "issue"; number: number } | null>(null);

  function loadPrDetail(repo: string, number: number) {
    return prDetail.load(`${repo}|${number}|${reloadNonce}`, () => api.githubPrDetail(repo, number));
  }
  function loadIssueDetail(repo: string, number: number) {
    return issueDetail.load(`${repo}|${number}|${reloadNonce}`, () => api.githubIssueDetail(repo, number));
  }

  function loadTraffic(repo: string) {
    return traffic.load(`${repo}|${reloadNonce}`, () => api.githubTraffic(repo));
  }
  function loadContributors(repo: string) {
    return contributors.load(`${repo}|${reloadNonce}`, () => api.githubContributors(repo, 12));
  }
  function loadActivity(repo: string) {
    return activity.load(`${repo}|${reloadNonce}`, async () => {
      // /stats/commit_activity returns 202 + empty body while GitHub computes;
      // retry a few times before giving up and showing the "computing" state.
      let a = await api.githubActivity(repo);
      for (let tries = 0; a.computing && tries < 3; tries++) {
        await new Promise((r) => setTimeout(r, 1800));
        a = await api.githubActivity(repo);
      }
      return a;
    });
  }
  function loadMilestones(repo: string) {
    return milestones.load(`${repo}|${reloadNonce}`, () => api.githubMilestones(repo));
  }
  function loadLabels(repo: string) {
    return labels.load(`${repo}|${reloadNonce}`, () => api.githubLabels(repo));
  }

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

  async function loadStats(repo: string, keepStale = false) {
    statsLoading = true;
    statsError = null;
    // On a soft refresh keep the prior numbers visible so the header tiles
    // don't blink to skeletons; only blank them on a fresh repo load.
    if (!keepStale) stats = null;
    try {
      stats = await api.githubRepoStats(repo);
    } catch (e) {
      statsError = e as GithubError;
    } finally {
      statsLoading = false;
    }
  }

  // Re-fetch availability + stats in place without blanking them, so a Refresh
  // of the already-loaded repo doesn't flash the header/tabs. The caller bumps
  // reloadNonce, which re-runs each tab's load-effect; makePanel.load retains
  // prior panel data while re-fetching (no skeleton, no re-played reveal).
  async function softRefresh(repo: string) {
    try {
      const a = await api.githubAvailability(repo);
      availability = a;
      if (a.kind === "Ok") void loadStats(repo, true);
    } catch {
      /* keep showing the prior availability on a transient failure */
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
    traffic.reset();
    contributors.reset();
    activity.reset();
    milestones.reset();
    labels.reset();
    selectedItem = null;
    prDetail.reset();
    issueDetail.reset();
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
      selectedItem = null;
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
      if (s === pullState) return;
      pullState = s;
      // A filter switch is a different query, so the current list is the wrong
      // answer — clear it so the skeleton + reveal play while the new filter
      // loads. (Refresh keeps stale data; a filter change should not.)
      pulls.reset();
    },
    get issueState() {
      return issueState;
    },
    setIssueState(s: IssueStateFilter) {
      if (s === issueState) return;
      issueState = s;
      // See setPullState: a filter switch invalidates the shown list.
      issues.reset();
    },
    /** Read by each tab's load-effect so a Refresh re-runs them. */
    get reloadNonce() {
      return reloadNonce;
    },
    loadPulls,
    loadIssues,
    loadReleases,
    loadRuns,
    get traffic() {
      return traffic;
    },
    get contributors() {
      return contributors;
    },
    get activity() {
      return activity;
    },
    get milestones() {
      return milestones;
    },
    get labels() {
      return labels;
    },
    loadTraffic,
    loadContributors,
    loadActivity,
    loadMilestones,
    loadLabels,
    get selectedItem() {
      return selectedItem;
    },
    openItem(kind: "pr" | "issue", number: number) {
      selectedItem = { kind, number };
    },
    closeItem() {
      selectedItem = null;
    },
    get prDetail() {
      return prDetail;
    },
    get issueDetail() {
      return issueDetail;
    },
    loadPrDetail,
    loadIssueDetail,
    ensure,
    refresh(repo: string) {
      if (!isTauri()) return;
      // A different (or never-loaded) repo gets a full reload with reset.
      if (loadedRepo !== repo || !availability) {
        loadedRepo = null;
        reloadNonce++;
        return ensure(repo);
      }
      // Same repo: soft refresh — keep stale data on screen while re-fetching.
      reloadNonce++;
      void softRefresh(repo);
    },
    /** Force the active tab's load-effect to re-fetch (used after a write action). */
    bumpReload() {
      reloadNonce++;
    },
  };
}

export const githubState = makeGithubState();
