// Shared singleton for the prerequisite probe (git / python3 / bundled git-filter-repo).
// Lifted out of PrereqBanner so the editing controls (ApplyPanel) gate on the SAME result
// instead of the banner merely *claiming* commit-time editing is disabled while the
// destructive "Rewrite history" action stays reachable. One probe, shared across callers;
// load() dedupes concurrent callers (banner + apply panel) and retries after a failure.
import { api } from "./api";
import type { PrerequisiteCheck } from "./types";

function makePrerequisites() {
  let check = $state<PrerequisiteCheck | null>(null);
  let loading: Promise<void> | null = null;

  async function probe() {
    try {
      check = await api.checkPrerequisites();
    } catch {
      check = null;
      // Clear the cached promise so a later load()/refresh() actually re-probes. Without
      // this, a transient failure leaves `loading` holding this (resolved) promise, so
      // load() no-ops forever and canEditHistory stays true — history editing wrongly
      // enabled, and the banner hidden, for the rest of the session.
      loading = null;
    }
  }

  return {
    get check() {
      return check;
    },
    // Commit-time editing needs git + python3 + a working bundled filter-repo. Unknown
    // (not yet probed / non-Tauri) → treated as available so we never block prematurely;
    // a genuinely-missing tool still fails loudly at run time.
    get canEditHistory() {
      return check ? check.git && check.python3 && check.filterRepo : true;
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
