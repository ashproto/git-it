// Pure copy + settings logic for the Pull dialog, the toolbar Pull button and the pull
// settings. Tauri-free so vitest covers every string the dialog can show.
import type { PullStrategy, Ref, RepoStatus } from "./types";

export const PULL_STRATEGIES: readonly PullStrategy[] = ["merge", "rebase", "ffOnly"];

// Radio labels in the dialog.
export const PULL_STRATEGY_LABELS: Record<PullStrategy, string> = {
  merge: "Merge",
  rebase: "Rebase",
  ffOnly: "Fast-forward only",
};

// One-line description under each radio.
export const PULL_STRATEGY_DESCRIPTIONS: Record<PullStrategy, string> = {
  merge: "Fast-forward when possible, otherwise create a merge commit",
  rebase: "Replay your commits on top of the remote's",
  ffOnly: "Stop if the branches have diverged",
};

// Lowercase form for running text ("Pull (fast-forward only)").
const PULL_STRATEGY_WORDS: Record<PullStrategy, string> = {
  merge: "merge",
  rebase: "rebase",
  ffOnly: "fast-forward only",
};

// Operation label for runRemote / the status bar: "Pull", "Pull (rebase)", …
export function pullLabel(strategy: PullStrategy): string {
  return strategy === "merge" ? "Pull" : `Pull (${PULL_STRATEGY_WORDS[strategy]})`;
}

// Toolbar tooltip. When the dialog is off, name the strategy a click will use and how
// to get the options back.
export function pullButtonTitle(askEveryTime: boolean, strategy: PullStrategy): string {
  return askEveryTime
    ? "Pull changes from remote"
    : `Pull (${PULL_STRATEGY_WORDS[strategy]}) — ⌥-click for options`;
}

// A pull needs a checked-out branch with an upstream that still exists to pull from.
// `upstreamGone` is required (no default) so that no caller can silently drop it.
export function canPull(
  branch: string | null,
  upstream: string | null,
  upstreamGone: boolean,
): boolean {
  return branch !== null && upstream !== null && !upstreamGone;
}

export function pullDialogTitle(branch: string | null, upstream: string | null): string {
  return branch !== null && upstream !== null ? `Pull ${upstream} → ${branch}` : "Pull";
}

// Status line under the title. Only non-zero counts are named; ahead/behind come from
// the tracking ref, i.e. the last fetch, so say so. A gone upstream's counts read 0/0,
// so it is ruled out before them. Typically a merged PR's branch was deleted, so the
// copy doesn't suggest pushing the branch again.
export function pullStatusLine(
  branch: string | null,
  upstream: string | null,
  ahead: number,
  behind: number,
  upstreamGone: boolean,
): string {
  if (branch === null) return "No branch is checked out.";
  if (upstream === null) return `${branch} has no upstream branch — push it first to set one.`;
  if (upstreamGone) {
    return `${upstream} no longer exists as of the last fetch — there's nothing to pull.`;
  }
  if (ahead > 0 && behind > 0) {
    return `${branch} is ${ahead} ahead, ${behind} behind ${upstream} as of the last fetch.`;
  }
  if (ahead > 0) return `${branch} is ${ahead} ahead of ${upstream} as of the last fetch.`;
  if (behind > 0) return `${branch} is ${behind} behind ${upstream} as of the last fetch.`;
  return `Up to date with ${upstream} as of the last fetch.`;
}

// Saved-settings migration. `saved` is the persisted pullStrategy (a string, or
// null/undefined when never saved); `legacyRebase` is the pre-dialog `pullRebase` flag
// (a boolean in the Tauri store, the string "true"/"false" in localStorage). A saved
// strategy wins; otherwise a legacy rebase preference becomes "rebase"; otherwise merge.
export function resolvePullStrategy(saved: unknown, legacyRebase: unknown): PullStrategy {
  if (saved === "merge" || saved === "rebase" || saved === "ffOnly") return saved;
  return legacyRebase === true || legacyRebase === "true" ? "rebase" : "merge";
}

export type PullContext = {
  branch: string | null;
  upstream: string | null;
  // The branch still names an upstream, but no such ref exists any more.
  upstreamGone: boolean;
  ahead: number;
  behind: number;
};

// What the dialog shows for the checked-out branch. The branch comes from `git status`
// (the repository status), NOT from graph decorations: a branch whose commit sits outside
// the loaded page carries no `is_head` decoration and would wrongly read as "no branch".
// Tracking data comes from the `list_refs` inventory entry for that local branch.
// Fetch prunes, so a branch deleted on the remote (typically a merged PR's) leaves its
// upstream name configured and its "[gone]" tracking read as 0/0, while the ref itself
// drops out of the inventory. Any non-tag ref counts, since an upstream can be local.
export function pullContext(status: RepoStatus | null, refs: Ref[]): PullContext {
  const branch = status && !status.head.detached ? status.head.branch : null;
  const tracking = branch
    ? (refs.find((r) => r.kind === "local" && r.name === branch) ?? null)
    : null;
  const upstream = tracking?.upstream ?? null;
  return {
    branch,
    upstream,
    upstreamGone: upstream !== null && !refs.some((r) => r.kind !== "tag" && r.name === upstream),
    ahead: tracking?.ahead ?? 0,
    behind: tracking?.behind ?? 0,
  };
}
