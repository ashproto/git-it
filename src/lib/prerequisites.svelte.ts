// Shared singleton for the prerequisite probe (git / python3 / bundled git-filter-repo).
// Lifted out of PrereqBanner so the editing controls (ApplyPanel) gate on the SAME result
// instead of the banner merely *claiming* commit-time editing is disabled while the
// destructive "Rewrite history" action stays reachable. One probe, shared across callers;
// load() dedupes concurrent callers (banner + apply panel) and retries after a failure.
import { api } from "./api";
import type { PrerequisiteCheck } from "./types";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function makePrerequisites() {
  let check = $state<PrerequisiteCheck | null>(null);
  // True once a probe has REJECTED without producing a result — distinct from "not probed
  // yet" (check null, probeFailed false). Keeps history editing disabled while set.
  let probeFailed = $state(false);
  let loading: Promise<void> | null = null;

  async function probe() {
    // Non-Tauri (browser preview) has no backend to probe — leave the state "unknown"
    // (not a failure) so canEditHistory stays optimistic there.
    if (!isTauri()) return;
    try {
      check = await api.checkPrerequisites();
      probeFailed = false;
    } catch {
      check = null;
      probeFailed = true;
      // Clear the cached promise so a later load()/refresh() re-probes a transient failure
      // instead of no-op'ing on this (resolved) promise for the rest of the session.
      loading = null;
    }
  }

  return {
    get check() {
      return check;
    },
    // True only when a SUCCESSFUL probe confirms git + python3 + a working bundled
    // filter-repo. Before the first probe resolves — and in non-Tauri — we stay optimistic
    // (check null, probeFailed false) so the control doesn't flash disabled. But once a
    // probe has FAILED we keep editing DISABLED: a destructive history rewrite must never
    // run on unverified prerequisites. A later successful probe re-enables it.
    get canEditHistory() {
      if (check) return check.git && check.python3 && check.filterRepo;
      return !probeFailed;
    },
    // Probe once, then share the result — deduped so the banner and ApplyPanel don't
    // double-invoke.
    load() {
      if (!loading) loading = probe();
      return loading;
    },
    // Force a fresh probe. Used when the user returns to the app after fixing a
    // prerequisite (e.g. finishing the async Command Line Tools installer), so the
    // banner clears and the editing controls re-enable without a restart.
    refresh() {
      loading = probe();
      return loading;
    },
  };
}

export const prerequisites = makePrerequisites();
