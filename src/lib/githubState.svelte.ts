import { appState } from "./store.svelte";
import { api } from "./api";
import { parseGithubRemote } from "./github/remote";
import type { GhAvailability, GhRepoStats, GithubError } from "./types";

export type GithubTab = "overview" | "pulls" | "issues" | "releases" | "actions" | "insights";

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
    ensure,
    refresh(repo: string) {
      loadedRepo = null;
      return ensure(repo);
    },
  };
}

export const githubState = makeGithubState();
