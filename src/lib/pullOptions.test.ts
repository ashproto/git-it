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
  samePullTarget,
  type PullContext,
  type PullSource,
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
  { name: "main", kind: "remote", target_sha: "b".repeat(40), upstream: null, ahead: 0, behind: 0, merge_source: null },
  { name: "main", kind: "local", target_sha: "a".repeat(40), upstream: "origin/main", ahead: 2, behind: 3, merge_source: "origin main" },
  { name: "origin/main", kind: "remote", target_sha: "b".repeat(40), upstream: null, ahead: 0, behind: 0, merge_source: null },
  { name: "lonely", kind: "local", target_sha: "c".repeat(40), upstream: null, ahead: 0, behind: 0, merge_source: null },
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
  // main tracking origin/main, `ahead`/`behind` apart as of the last fetch.
  const tracking = (ahead: number, behind: number): PullContext => ({
    branch: "main",
    source: { kind: "tracked", upstream: "origin/main", gone: false, ahead, behind },
  });
  const none: PullSource = { kind: "none" };

  it("titles the dialog with the upstream and the branch", () => {
    expect(pullDialogTitle(tracking(2, 3))).toBe("Pull origin/main → main");
    expect(pullDialogTitle({ branch: "main", source: none })).toBe("Pull");
    expect(pullDialogTitle({ branch: null, source: none })).toBe("Pull");
  });

  it("names only the non-zero counts", () => {
    expect(pullStatusLine(tracking(2, 3))).toBe(
      "main is 2 ahead, 3 behind origin/main as of the last fetch.",
    );
    expect(pullStatusLine(tracking(0, 3))).toBe(
      "main is 3 behind origin/main as of the last fetch.",
    );
    expect(pullStatusLine(tracking(2, 0))).toBe(
      "main is 2 ahead of origin/main as of the last fetch.",
    );
    expect(pullStatusLine(tracking(0, 0))).toBe(
      "Up to date with origin/main as of the last fetch.",
    );
  });

  it("explains why a pull is impossible", () => {
    expect(pullStatusLine({ branch: "main", source: none })).toBe(
      "main has no upstream branch — push it first to set one.",
    );
    expect(pullStatusLine({ branch: null, source: none })).toBe("No branch is checked out.");
    expect(canPull(tracking(0, 0))).toBe(true);
    expect(canPull({ branch: "main", source: none })).toBe(false);
    expect(canPull({ branch: null, source: none })).toBe(false);
  });

  it("blocks a pull from an upstream the last fetch pruned", () => {
    const pruned: PullContext = {
      branch: "feature",
      source: { kind: "tracked", upstream: "origin/feature", gone: true, ahead: 0, behind: 0 },
    };
    expect(pullStatusLine(pruned)).toBe(
      "origin/feature no longer exists as of the last fetch — there's nothing to pull.",
    );
    expect(canPull(pruned)).toBe(false);
  });

  it("pulls from a configured source that no tracking ref mirrors, without claiming counts", () => {
    // A fork's pull request checked out by `gh pr checkout`: git pulls refs/pull/7/head from
    // origin, but nothing mirrors it locally, so ahead/behind can't be counted.
    const pr: PullContext = {
      branch: "pr-7",
      source: { kind: "configured", source: "origin refs/pull/7/head" },
    };
    expect(canPull(pr)).toBe(true);
    expect(pullDialogTitle(pr)).toBe("Pull origin refs/pull/7/head → pr-7");
    expect(pullStatusLine(pr)).toBe(
      "pr-7 pulls from origin refs/pull/7/head, as configured — ahead/behind isn't known.",
    );
  });

  it("leaves the upstream to git when the branch list couldn't be read", () => {
    const unknown: PullContext = { branch: "main", source: { kind: "unknown" } };
    expect(canPull(unknown)).toBe(true);
    expect(pullDialogTitle(unknown)).toBe("Pull");
    expect(pullStatusLine(unknown)).toBe(
      "main's upstream isn't known — git will pull using its own configuration.",
    );
  });

  it("requires the whole context, so no caller can silently drop part of it", () => {
    // Compile-time only, enforced by `npm run check`; never called, as the calls are invalid.
    const invalid = () => {
      // @ts-expect-error: where the branch pulls from is required
      canPull({ branch: "main" });
      // @ts-expect-error: a tracked upstream must say whether it still exists
      pullStatusLine({ branch: "main", source: { kind: "tracked", upstream: "origin/main", ahead: 0, behind: 0 } });
      // @ts-expect-error: a configured source has no counts to show
      pullStatusLine({ branch: "main", source: { kind: "configured", source: "origin x", ahead: 0 } });
      // @ts-expect-error: whether the inventory is known is required
      pullContext(status, refs);
    };
    expect(invalid).toBeTypeOf("function");
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
    merge_source: "origin feature",
  };

  it("takes the branch from the repository status and tracking data from the local ref", () => {
    expect(pullContext(status, refs, true)).toEqual({
      branch: "main",
      source: { kind: "tracked", upstream: "origin/main", gone: false, ahead: 2, behind: 3 },
    });
    expect(pullContext({ ...status, head: { ...status.head, branch: "lonely" } }, refs, true)).toEqual({
      branch: "lonely",
      source: { kind: "none" },
    });
  });

  it("marks an upstream missing from the inventory as gone", () => {
    expect(pullContext(onBranch("feature"), [...refs, feature], true)).toEqual({
      branch: "feature",
      source: { kind: "tracked", upstream: "origin/feature", gone: true, ahead: 0, behind: 0 },
    });
  });

  it("counts a local branch as a live upstream, but not a tag", () => {
    // `git branch -u lonely` makes a local branch the upstream.
    const topic: Ref = { ...feature, name: "topic", upstream: "lonely" };
    expect(pullContext(onBranch("topic"), [...refs, topic], true).source).toMatchObject({
      kind: "tracked",
      gone: false,
    });
    const tag: Ref = { ...feature, name: "origin/feature", kind: "tag", upstream: null };
    expect(pullContext(onBranch("feature"), [...refs, feature, tag], true).source).toMatchObject({
      kind: "tracked",
      gone: true,
    });
  });

  it("reads a merge source that no tracking ref mirrors as configured", () => {
    const pr: Ref = { ...feature, name: "pr-7", upstream: null, merge_source: "origin refs/pull/7/head" };
    expect(pullContext(onBranch("pr-7"), [...refs, pr], true)).toEqual({
      branch: "pr-7",
      source: { kind: "configured", source: "origin refs/pull/7/head" },
    });
  });

  it("keeps a tracked upstream over its merge source, so a pruned one stays gone", () => {
    // `feature` names origin feature as its merge source too, as every tracked branch does.
    expect(feature.merge_source).toBe("origin feature");
    expect(pullContext(onBranch("feature"), [...refs, feature], true).source).toMatchObject({
      kind: "tracked",
      gone: true,
    });
  });

  it("calls the source unknown, not missing, while the inventory is unknown", () => {
    expect(pullContext(status, [], false)).toEqual({ branch: "main", source: { kind: "unknown" } });
    expect(pullContext(onBranch("lonely"), refs, false).source).toEqual({ kind: "unknown" });
    // No branch is still no branch.
    const detached = { ...status, head: { sha: "a".repeat(40), branch: null, detached: true } };
    expect(pullContext(detached, [], false)).toEqual({ branch: null, source: { kind: "none" } });
  });

  it("blocks only a branch with nothing to pull from", () => {
    const pr: Ref = { ...feature, name: "pr-7", upstream: null, merge_source: "origin refs/pull/7/head" };
    const inventory = [...refs, pr];
    expect(canPull(pullContext(status, inventory, true))).toBe(true); // tracked
    expect(canPull(pullContext(onBranch("pr-7"), inventory, true))).toBe(true); // configured
    expect(canPull(pullContext(onBranch("lonely"), inventory, false))).toBe(true); // unknown
    expect(canPull(pullContext(onBranch("lonely"), inventory, true))).toBe(false); // unconfigured
  });

  it("reports no branch for a detached HEAD or an unknown status, and keeps the branch without an upstream for an empty inventory", () => {
    const detached = { ...status, head: { sha: "a".repeat(40), branch: null, detached: true } };
    expect(pullContext(detached, refs, true)).toEqual({ branch: null, source: { kind: "none" } });
    expect(pullContext(null, refs, true)).toEqual({ branch: null, source: { kind: "none" } });
    expect(pullContext(status, [], true)).toEqual({ branch: "main", source: { kind: "none" } });
  });
});

describe("samePullTarget (what a confirmed pull is bound to)", () => {
  const onMain = (source: PullSource): PullContext => ({ branch: "main", source });
  const tracked = (upstream: string, gone = false, ahead = 0): PullSource => ({
    kind: "tracked",
    upstream,
    gone,
    ahead,
    behind: 0,
  });
  const configured = (source: string): PullSource => ({ kind: "configured", source });

  it("ignores what a fetch in between says about the same upstream", () => {
    expect(samePullTarget(onMain(tracked("origin/main")), onMain(tracked("origin/main", true, 4))))
      .toBe(true);
    expect(samePullTarget(onMain(configured("origin pr")), onMain(configured("origin pr"))))
      .toBe(true);
    expect(samePullTarget(onMain({ kind: "unknown" }), onMain({ kind: "unknown" }))).toBe(true);
  });

  it("tells apart another branch, another upstream, another merge source or another kind", () => {
    const main = onMain(tracked("origin/main"));
    expect(samePullTarget(main, { ...main, branch: "feature" })).toBe(false);
    expect(samePullTarget(main, onMain(tracked("upstream/main")))).toBe(false);
    expect(
      samePullTarget(onMain(configured("origin refs/pull/1/head")), onMain(configured("origin refs/pull/2/head"))),
    ).toBe(false);
    expect(samePullTarget(main, onMain(configured("origin main")))).toBe(false);
    expect(samePullTarget(main, onMain({ kind: "unknown" }))).toBe(false);
    expect(samePullTarget(main, onMain({ kind: "none" }))).toBe(false);
  });
});
