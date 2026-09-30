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

// A pull needs a checked-out branch and somewhere to pull from that still exists. A source
// the inventory can't vouch for (configured, unknown) is git's to judge.
export function canPull(ctx: PullContext): boolean {
  if (ctx.branch === null) return false;
  switch (ctx.source.kind) {
    case "tracked":
      return !ctx.source.gone;
    case "configured":
    case "unknown":
      return true;
    case "none":
      return false;
  }
}

export function pullDialogTitle(ctx: PullContext): string {
  if (ctx.branch === null) return "Pull";
  switch (ctx.source.kind) {
    case "tracked":
      return `Pull ${ctx.source.upstream} → ${ctx.branch}`;
    case "configured":
      return `Pull ${ctx.source.source} → ${ctx.branch}`;
    case "unknown":
    case "none":
      return "Pull";
  }
}

// Status line under the title. Only non-zero counts are named; ahead/behind come from
// the tracking ref, i.e. the last fetch, so say so. A gone upstream's counts read 0/0,
// so it is ruled out before them. Typically a merged PR's branch was deleted, so the
// copy doesn't suggest pushing the branch again. Without a tracking ref there are no
// counts at all, so none are claimed.
export function pullStatusLine(ctx: PullContext): string {
  const { branch, source } = ctx;
  if (branch === null) return "No branch is checked out.";
  switch (source.kind) {
    case "none":
      return `${branch} has no upstream branch — push it first to set one.`;
    case "unknown":
      return `${branch}'s upstream isn't known — git will pull using its own configuration.`;
    case "configured":
      return `${branch} pulls from ${source.source}, as configured — ahead/behind isn't known.`;
    case "tracked": {
      const { upstream, gone, ahead, behind } = source;
      if (gone) return `${upstream} no longer exists as of the last fetch — there's nothing to pull.`;
      if (ahead > 0 && behind > 0) {
        return `${branch} is ${ahead} ahead, ${behind} behind ${upstream} as of the last fetch.`;
      }
      if (ahead > 0) return `${branch} is ${ahead} ahead of ${upstream} as of the last fetch.`;
      if (behind > 0) return `${branch} is ${behind} behind ${upstream} as of the last fetch.`;
      return `Up to date with ${upstream} as of the last fetch.`;
    }
  }
}

// Saved-settings migration. `saved` is the persisted pullStrategy (a string, or
// null/undefined when never saved); `legacyRebase` is the pre-dialog `pullRebase` flag
// (a boolean in the Tauri store, the string "true"/"false" in localStorage). A saved
// strategy wins; otherwise a legacy rebase preference becomes "rebase"; otherwise merge.
export function resolvePullStrategy(saved: unknown, legacyRebase: unknown): PullStrategy {
  if (saved === "merge" || saved === "rebase" || saved === "ffOnly") return saved;
  return legacyRebase === true || legacyRebase === "true" ? "rebase" : "merge";
}

// Where the checked-out branch pulls from, as far as the ref inventory tells. The helpers here
// switch on `kind` exhaustively, so a new kind is a compile error until each one handles it.
export type PullSource =
  // A ref mirrors the upstream (`upstream`, e.g. "origin/main"), so the counts come from it,
  // as of the last fetch. `gone`: the branch still names the upstream, but no such ref exists.
  | { kind: "tracked"; upstream: string; gone: boolean; ahead: number; behind: number }
  // The branch's `merge_source` (e.g. "origin refs/pull/7/head"), which no ref mirrors — a
  // fork's PR, another branch of a single-branch clone. git pulls it; nothing counts it.
  | { kind: "configured"; source: string }
  // The inventory isn't loaded for this repository, or failed to load: git decides.
  | { kind: "unknown" }
  // Nothing to pull from: git would refuse.
  | { kind: "none" };

export type PullContext = {
  branch: string | null;
  source: PullSource;
};

// What a source says the pull is from — not whether it still exists, nor the counts.
function pullSourceKey(source: PullSource): string {
  switch (source.kind) {
    case "tracked":
      return `tracked ${source.upstream}`;
    case "configured":
      return `configured ${source.source}`;
    case "unknown":
    case "none":
      return source.kind;
  }
}

// The same pull: the same branch, pulling from the same place. A fetch in between may still
// have pruned the upstream or moved the counts; canPull and the status line judge those.
export function samePullTarget(a: PullContext, b: PullContext): boolean {
  return a.branch === b.branch && pullSourceKey(a.source) === pullSourceKey(b.source);
}

// What the dialog shows for the checked-out branch. The branch comes from `git status`
// (the repository status), NOT from graph decorations: a branch whose commit sits outside
// the loaded page carries no `is_head` decoration and would wrongly read as "no branch".
// Tracking data comes from the `list_refs` inventory entry for that local branch, when
// `inventoryKnown` says `refs` is that inventory: an empty list proves nothing otherwise.
// Fetch prunes, so a branch deleted on the remote (typically a merged PR's) leaves its
// upstream name configured and its "[gone]" tracking read as 0/0, while the ref itself
// drops out of the inventory. Any non-tag ref counts, since an upstream can be local.
// Only a tracked upstream can be gone: a configured source may exist on the remote alone.
export function pullContext(
  status: RepoStatus | null,
  refs: Ref[],
  inventoryKnown: boolean,
): PullContext {
  const branch = status && !status.head.detached ? status.head.branch : null;
  if (branch === null) return { branch, source: { kind: "none" } };
  if (!inventoryKnown) return { branch, source: { kind: "unknown" } };
  const tracking = refs.find((r) => r.kind === "local" && r.name === branch) ?? null;
  const upstream = tracking?.upstream ?? null;
  if (tracking === null || upstream === null) {
    const source = tracking?.merge_source ?? null;
    return { branch, source: source === null ? { kind: "none" } : { kind: "configured", source } };
  }
  return {
    branch,
    source: {
      kind: "tracked",
      upstream,
      gone: !refs.some((r) => r.kind !== "tag" && r.name === upstream),
      ahead: tracking.ahead,
      behind: tracking.behind,
    },
  };
}
