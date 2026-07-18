import { describe, expect, it } from "vitest";
import { dialogs } from "./dialogs.svelte";
import type { WorktreeInfo } from "./types";

function cleanLinkedWorktree(): WorktreeInfo {
  return {
    path: "/tmp/git-it-linked-worktree",
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
  };
}

describe("linked-worktree branch deletion", () => {
  it("cannot confirm until removal is explicitly selected", async () => {
    const result = dialogs.confirmBranchDelete({
      branch: "feature",
      upstream: null,
      worktree: cleanLinkedWorktree(),
    });

    expect(dialogs.state.kind).toBe("branchDelete");
    if (dialogs.state.kind !== "branchDelete") throw new Error("branch dialog did not open");
    expect(dialogs.state.removeWorktree).toBe(false);

    dialogs.resolveBranchDelete(true);
    expect(dialogs.state.kind).toBe("branchDelete");

    dialogs.setBranchDeleteWorktree(true);
    dialogs.resolveBranchDelete(true);

    await expect(result).resolves.toEqual({
      confirmed: true,
      force: false,
      deleteRemote: false,
      removeWorktree: true,
    });
    expect(dialogs.state.kind).toBe("none");
  });
});
