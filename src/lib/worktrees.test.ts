import { describe, expect, it } from "vitest";
import type { WorktreeInfo } from "./types";
import {
  worktreeForBranch,
  worktreeIsDirty,
  worktreeRemovalBlocker,
  worktreeStatusLabel,
} from "./worktrees";

function worktree(overrides: Partial<WorktreeInfo> = {}): WorktreeInfo {
  return {
    path: "/tmp/repo-worktree",
    head: "a".repeat(40),
    branch: "feature",
    isMain: false,
    isCurrent: false,
    detached: false,
    bare: false,
    locked: false,
    lockedReason: null,
    prunable: false,
    prunableReason: null,
    status: {
      head: { sha: "a".repeat(40), branch: "feature", detached: false },
      staged: 0,
      unstaged: 0,
      untracked: 0,
      conflicted: 0,
      operation: null,
    },
    ...overrides,
  };
}

describe("worktree helpers", () => {
  it("maps a local branch to its worktree", () => {
    const feature = worktree();
    expect(worktreeForBranch([feature], "feature")).toBe(feature);
    expect(worktreeForBranch([feature], "main")).toBeUndefined();
  });

  it("treats file changes, conflicts, and in-progress operations as dirty", () => {
    expect(worktreeIsDirty(worktree())).toBe(false);
    expect(worktreeIsDirty(worktree({ status: { ...worktree().status!, untracked: 1 } }))).toBe(true);
    expect(worktreeIsDirty(worktree({ status: { ...worktree().status!, operation: "rebase" } }))).toBe(true);
    expect(worktreeIsDirty(worktree({ status: { ...worktree().status!, operation: "bisect" } }))).toBe(true);
  });

  it("allows only a verified clean linked worktree to be removed", () => {
    expect(worktreeRemovalBlocker(worktree())).toBeNull();
    expect(worktreeRemovalBlocker(worktree({ isCurrent: true }))).toContain("currently open");
    expect(worktreeRemovalBlocker(worktree({ isMain: true }))).toContain("main worktree");
    expect(worktreeRemovalBlocker(worktree({ locked: true, lockedReason: "in use" }))).toContain("in use");
    expect(worktreeRemovalBlocker(worktree({ prunable: true }))).toContain("stale or missing");
    expect(worktreeRemovalBlocker(worktree({ status: null }))).toContain("could not verify");
    expect(
      worktreeRemovalBlocker(worktree({ status: { ...worktree().status!, unstaged: 1 } })),
    ).toContain("uncommitted changes");
    expect(
      worktreeRemovalBlocker(worktree({ status: { ...worktree().status!, operation: "bisect" } })),
    ).toContain("operation in progress");
  });

  it("prioritizes the most useful status label", () => {
    expect(worktreeStatusLabel(worktree({ isCurrent: true, locked: true }))).toBe("Current");
    expect(worktreeStatusLabel(worktree({ locked: true }))).toBe("Locked");
    expect(worktreeStatusLabel(worktree({ prunable: true }))).toBe("Missing");
    expect(worktreeStatusLabel(worktree({ status: { ...worktree().status!, staged: 1 } }))).toBe("Modified");
    expect(worktreeStatusLabel(worktree())).toBeNull();
  });
});
