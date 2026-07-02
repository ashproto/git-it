// Pure branch↔PR matcher for the git↔GitHub bridges (branch context menu,
// title-bar PR chip). Kept Tauri-free so vitest can cover it.
import type { GhPull } from "../types";

/** The OPEN pull request whose head branch is exactly `branch`, from the cached
 *  pulls list. The cache may hold ANY state filter's results (open/closed/
 *  merged/all), so the state is always checked — gh emits list states in
 *  UPPERCASE ("OPEN"). Returns null when the list hasn't loaded, the branch is
 *  empty, or nothing matches. */
export function prForBranch(pulls: GhPull[] | null, branch: string): GhPull | null {
  if (!pulls || !branch) return null;
  return pulls.find((p) => p.state === "OPEN" && p.headRefName === branch) ?? null;
}
