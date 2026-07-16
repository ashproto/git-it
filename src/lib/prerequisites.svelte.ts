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
    load() {
      if (loading) return loading;
      loading = (async () => {
        try {
          check = await api.checkPrerequisites();
        } catch {
          check = null;
          loading = null; // allow a retry after a transient probe failure
        }
      })();
      return loading;
    },
  };
}

export const prerequisites = makePrerequisites();
