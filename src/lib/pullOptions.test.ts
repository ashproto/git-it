import { describe, expect, it } from "vitest";
import {
  PULL_STRATEGIES,
  PULL_STRATEGY_DESCRIPTIONS,
  PULL_STRATEGY_LABELS,
  canPull,
  pullButtonTitle,
  pullContext,
  pullDialogTitle,
  pullLabel,
  pullStatusLine,
  resolvePullStrategy,
} from "./pullOptions";
import type { Ref, RepoStatus } from "./types";

const status: RepoStatus = {
  head: { sha: "a".repeat(40), branch: "main", detached: false },
  staged: 0,
  unstaged: 0,
  untracked: 0,
  conflicted: 0,
  operation: null,
};
const refs: Ref[] = [
  // Shares the local branch's name without being a local branch. It comes first so that a
  // lookup ignoring the kind would pick it up.
  { name: "main", kind: "remote", target_sha: "b".repeat(40), upstream: null, ahead: 0, behind: 0 },
  { name: "main", kind: "local", target_sha: "a".repeat(40), upstream: "origin/main", ahead: 2, behind: 3 },
  { name: "origin/main", kind: "remote", target_sha: "b".repeat(40), upstream: null, ahead: 0, behind: 0 },
  { name: "lonely", kind: "local", target_sha: "c".repeat(40), upstream: null, ahead: 0, behind: 0 },
];

describe("pull labels", () => {
  it("names the strategy in the operation label, except for the default merge", () => {
    expect(pullLabel("merge")).toBe("Pull");
    expect(pullLabel("rebase")).toBe("Pull (rebase)");
    expect(pullLabel("ffOnly")).toBe("Pull (fast-forward only)");
  });

  it("lists every strategy with a radio label and a one-line description", () => {
    expect(PULL_STRATEGIES).toEqual(["merge", "rebase", "ffOnly"]);
    expect(PULL_STRATEGY_LABELS).toEqual({
      merge: "Merge",
      rebase: "Rebase",
      ffOnly: "Fast-forward only",
    });
    expect(PULL_STRATEGY_DESCRIPTIONS).toEqual({
      merge: "Fast-forward when possible, otherwise create a merge commit",
      rebase: "Replay your commits on top of the remote's",
      ffOnly: "Stop if the branches have diverged",
    });
  });

  it("explains the toolbar button only when the dialog is off", () => {
    expect(pullButtonTitle(true, "rebase")).toBe("Pull changes from remote");
    expect(pullButtonTitle(false, "merge")).toBe("Pull (merge) — ⌥-click for options");
    expect(pullButtonTitle(false, "ffOnly")).toBe(
      "Pull (fast-forward only) — ⌥-click for options",
    );
  });
});

describe("pull dialog copy", () => {
  it("titles the dialog with the upstream and the branch", () => {
    expect(pullDialogTitle("main", "origin/main")).toBe("Pull origin/main → main");
    expect(pullDialogTitle("main", null)).toBe("Pull");
    expect(pullDialogTitle(null, null)).toBe("Pull");
  });

  it("names only the non-zero counts", () => {
    expect(pullStatusLine("main", "origin/main", 2, 3, false)).toBe(
      "main is 2 ahead, 3 behind origin/main as of the last fetch.",
    );
    expect(pullStatusLine("main", "origin/main", 0, 3, false)).toBe(
      "main is 3 behind origin/main as of the last fetch.",
    );
    expect(pullStatusLine("main", "origin/main", 2, 0, false)).toBe(
      "main is 2 ahead of origin/main as of the last fetch.",
    );
    expect(pullStatusLine("main", "origin/main", 0, 0, false)).toBe(
      "Up to date with origin/main as of the last fetch.",
    );
  });

  it("explains why a pull is impossible", () => {
    expect(pullStatusLine("main", null, 0, 0, false)).toBe(
      "main has no upstream branch — push it first to set one.",
    );
    expect(pullStatusLine(null, null, 0, 0, false)).toBe("No branch is checked out.");
    expect(canPull("main", "origin/main", false)).toBe(true);
    expect(canPull("main", null, false)).toBe(false);
    expect(canPull(null, null, false)).toBe(false);
  });

  it("blocks a pull from an upstream the last fetch pruned", () => {
    expect(pullStatusLine("feature", "origin/feature", 0, 0, true)).toBe(
      "origin/feature no longer exists as of the last fetch — there's nothing to pull.",
    );
    expect(canPull("feature", "origin/feature", true)).toBe(false);
  });

  it("requires the gone flag, so no caller can silently drop it", () => {
    // Compile-time only, enforced by `npm run check`: leaving the flag out must not type-check.
    // @ts-expect-error: upstreamGone is required
    canPull("main", "origin/main");
    // @ts-expect-error: upstreamGone is required
    pullStatusLine("main", "origin/main", 0, 0);
  });
});

describe("resolvePullStrategy (settings migration)", () => {
  it("keeps a saved strategy over the legacy flag", () => {
    expect(resolvePullStrategy("ffOnly", true)).toBe("ffOnly");
    expect(resolvePullStrategy("merge", "true")).toBe("merge");
  });

  it("promotes the legacy pullRebase flag when nothing newer is saved", () => {
    expect(resolvePullStrategy(null, true)).toBe("rebase"); // Tauri store boolean
    expect(resolvePullStrategy(undefined, "true")).toBe("rebase"); // localStorage string
  });

  it("defaults to merge", () => {
    expect(resolvePullStrategy(null, null)).toBe("merge");
    expect(resolvePullStrategy(undefined, false)).toBe("merge");
    expect(resolvePullStrategy("bogus", "false")).toBe("merge");
  });
});

describe("pullContext (what the dialog shows)", () => {
  const onBranch = (branch: string): RepoStatus => ({ ...status, head: { ...status.head, branch } });
  // The remote deleted this branch (typically a merged PR's), so the fetch pruned
  // origin/feature, and its "[gone]" tracking parses to 0 ahead / 0 behind.
  const feature: Ref = {
    name: "feature",
    kind: "local",
    target_sha: "d".repeat(40),
    upstream: "origin/feature",
    ahead: 0,
    behind: 0,
  };

  it("takes the branch from the repository status and tracking data from the local ref", () => {
    expect(pullContext(status, refs)).toEqual({
      branch: "main",
      upstream: "origin/main",
      upstreamGone: false,
      ahead: 2,
      behind: 3,
    });
    expect(pullContext({ ...status, head: { ...status.head, branch: "lonely" } }, refs)).toEqual({
      branch: "lonely",
      upstream: null,
      upstreamGone: false,
      ahead: 0,
      behind: 0,
    });
  });

  it("marks an upstream missing from the inventory as gone", () => {
    expect(pullContext(onBranch("feature"), [...refs, feature])).toEqual({
      branch: "feature",
      upstream: "origin/feature",
      upstreamGone: true,
      ahead: 0,
      behind: 0,
    });
  });

  it("counts a local branch as a live upstream, but not a tag", () => {
    // `git branch -u lonely` makes a local branch the upstream.
    const topic: Ref = { ...feature, name: "topic", upstream: "lonely" };
    expect(pullContext(onBranch("topic"), [...refs, topic]).upstreamGone).toBe(false);
    const tag: Ref = { ...feature, name: "origin/feature", kind: "tag", upstream: null };
    expect(pullContext(onBranch("feature"), [...refs, feature, tag]).upstreamGone).toBe(true);
  });

  it("reports no branch for a detached HEAD or an unknown status, and keeps the branch without an upstream for an empty inventory", () => {
    const detached = { ...status, head: { sha: "a".repeat(40), branch: null, detached: true } };
    expect(pullContext(detached, refs)).toEqual({
      branch: null,
      upstream: null,
      upstreamGone: false,
      ahead: 0,
      behind: 0,
    });
    expect(pullContext(null, refs)).toEqual({
      branch: null,
      upstream: null,
      upstreamGone: false,
      ahead: 0,
      behind: 0,
    });
    expect(pullContext(status, [])).toEqual({
      branch: "main",
      upstream: null,
      upstreamGone: false,
      ahead: 0,
      behind: 0,
    });
  });
});
